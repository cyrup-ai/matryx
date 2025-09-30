//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use chrono::{DateTime, Duration, Utc};
use matryx_surrealdb::repository::PerformanceRepository;
use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::engine::any::Any;
use tracing::{info, warn};

use crate::federation::device_management::{DeviceError, DeviceListCache};

/// Efficient device cache manager with TTL and size limits
pub struct DeviceCacheManager {
    cache: HashMap<String, DeviceListCache>,
    cache_expiry: HashMap<String, DateTime<Utc>>,
    max_cache_size: usize,
    cache_ttl: Duration,
    hit_count: u64,
    miss_count: u64,
    performance_repo: Arc<PerformanceRepository<Any>>,
}

impl DeviceCacheManager {
    /// Create a new device cache manager
    pub fn new(
        max_cache_size: usize,
        cache_ttl_minutes: i64,
        performance_repo: Arc<PerformanceRepository<Any>>,
    ) -> Self {
        Self {
            cache: HashMap::new(),
            cache_expiry: HashMap::new(),
            max_cache_size,
            cache_ttl: Duration::minutes(cache_ttl_minutes),
            hit_count: 0,
            miss_count: 0,
            performance_repo,
        }
    }

    /// Get device list from cache or fetch if needed
    pub async fn get_device_list(&mut self, user_id: &str) -> Result<DeviceListCache, DeviceError> {
        // Check cache validity
        if let Some(expiry) = self.cache_expiry.get(user_id)
            && Utc::now() > *expiry {
                self.cache.remove(user_id);
                self.cache_expiry.remove(user_id);
                info!("Expired cache entry for user: {}", user_id);
            }

        let is_cache_hit = self.cache.contains_key(user_id);

        if !is_cache_hit {
            self.miss_count += 1;
            self.fetch_and_cache_device_list(user_id).await?;
        } else {
            self.hit_count += 1;
        }

        // Record cache performance in repository
        if let Err(e) = self
            .performance_repo
            .record_cache_hit_rate("device_cache", self.hit_count, self.miss_count)
            .await
        {
            warn!("Failed to record cache hit rate: {}", e);
        }

        self.cache
            .get(user_id)
            .cloned()
            .ok_or(DeviceError::DatabaseError("Cache miss".to_string()))
    }

    /// Fetch device list and cache it
    async fn fetch_and_cache_device_list(&mut self, user_id: &str) -> Result<(), DeviceError> {
        // Enforce cache size limits
        self.evict_if_necessary();

        // In a real implementation, this would fetch from the federation client
        // For now, we'll create an empty cache entry
        let cache_entry = DeviceListCache::new();

        self.cache.insert(user_id.to_string(), cache_entry);
        self.cache_expiry.insert(user_id.to_string(), Utc::now() + self.cache_ttl);

        info!("Cached device list for user: {}", user_id);
        Ok(())
    }

    /// Evict oldest entries if cache is full
    fn evict_if_necessary(&mut self) {
        if self.cache.len() >= self.max_cache_size {
            // Find the oldest entry by expiry time
            if let Some((oldest_user, _)) = self
                .cache_expiry
                .iter()
                .min_by_key(|(_, expiry)| *expiry)
                .map(|(user, expiry)| (user.clone(), *expiry))
            {
                self.cache.remove(&oldest_user);
                self.cache_expiry.remove(&oldest_user);
                info!("Evicted cache entry for user: {} (cache full)", oldest_user);
            }
        }
    }

    /// Manually invalidate cache for a user
    pub fn invalidate_user_cache(&mut self, user_id: &str) {
        if self.cache.remove(user_id).is_some() {
            self.cache_expiry.remove(user_id);
            info!("Manually invalidated cache for user: {}", user_id);
        }
    }

    /// Update an existing cache entry
    pub fn update_cache_entry(&mut self, user_id: &str, cache_entry: DeviceListCache) {
        if self.cache.contains_key(user_id) {
            self.cache.insert(user_id.to_string(), cache_entry);
            self.cache_expiry.insert(user_id.to_string(), Utc::now() + self.cache_ttl);
            info!("Updated cache entry for user: {}", user_id);
        }
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> DeviceCacheStats {
        let total_requests = self.hit_count + self.miss_count;
        let hit_ratio = if total_requests > 0 {
            self.hit_count as f64 / total_requests as f64
        } else {
            0.0
        };

        let stats = DeviceCacheStats {
            cache_size: self.cache.len(),
            max_cache_size: self.max_cache_size,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            hit_ratio,
            expired_entries: self.count_expired_entries(),
        };

        // Record memory usage (estimate based on cache size)
        let estimated_memory_mb = (self.cache.len() as f64 * 2.0) / 1024.0; // Rough estimate: 2KB per entry
        if let Err(e) = self
            .performance_repo
            .record_memory_usage("device_cache", estimated_memory_mb)
            .await
        {
            warn!("Failed to record memory usage: {}", e);
        }

        stats
    }

    /// Count entries that have expired but not yet been cleaned up
    fn count_expired_entries(&self) -> usize {
        let now = Utc::now();
        self.cache_expiry.values().filter(|expiry| now > **expiry).count()
    }

    /// Clean up expired entries (maintenance operation)
    pub fn cleanup_expired_entries(&mut self) {
        let now = Utc::now();
        let expired_users: Vec<String> = self
            .cache_expiry
            .iter()
            .filter(|(_, expiry)| now > **expiry)
            .map(|(user, _)| user.clone())
            .collect();

        let expired_count = expired_users.len();

        for user in expired_users {
            self.cache.remove(&user);
            self.cache_expiry.remove(&user);
        }

        if expired_count > 0 {
            info!("Cleaned up {} expired cache entries", expired_count);
        }
    }

    /// Clear all cache entries
    pub fn clear_cache(&mut self) {
        let count = self.cache.len();
        self.cache.clear();
        self.cache_expiry.clear();
        info!("Cleared all cache entries ({})", count);
    }

    /// Resize cache capacity
    pub fn resize_cache(&mut self, new_max_size: usize) {
        self.max_cache_size = new_max_size;

        // If new size is smaller, evict entries
        while self.cache.len() > new_max_size {
            self.evict_if_necessary();
        }

        info!("Resized cache to max {} entries", new_max_size);
    }
}

impl Default for DeviceCacheManager {
    fn default() -> Self {
        // Note: This default implementation creates a dummy repository
        // In practice, this should be injected with a real database connection
        let db = surrealdb::Surreal::init();
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        Self::new(1000, 60, performance_repo) // 1000 entries, 60 minutes TTL
    }
}

/// Cache performance statistics
#[derive(Debug, Clone)]
pub struct DeviceCacheStats {
    pub cache_size: usize,
    pub max_cache_size: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_ratio: f64,
    pub expired_entries: usize,
}

/// Batch device list fetcher for efficient bulk operations
pub struct BatchDeviceFetcher {
    cache_manager: DeviceCacheManager,
}

impl BatchDeviceFetcher {
    pub fn new(cache_manager: DeviceCacheManager) -> Self {
        Self { cache_manager }
    }

    /// Fetch device lists for multiple users efficiently
    pub async fn fetch_device_lists_batch(
        &mut self,
        user_ids: Vec<String>,
    ) -> Result<HashMap<String, DeviceListCache>, DeviceError> {
        let mut results = HashMap::new();
        let mut missing_users = Vec::new();

        // Check cache first
        for user_id in &user_ids {
            match self.cache_manager.get_device_list(user_id).await {
                Ok(cache) => {
                    results.insert(user_id.clone(), cache);
                },
                Err(_) => {
                    missing_users.push(user_id.clone());
                },
            }
        }

        // Fetch missing users in batch (would be implemented with federation client)
        if !missing_users.is_empty() {
            info!("Batch fetching device lists for {} users", missing_users.len());

            for user_id in missing_users {
                // Simulate fetching - in real implementation would batch the federation calls
                let cache_entry = DeviceListCache::new();
                self.cache_manager.update_cache_entry(&user_id, cache_entry.clone());
                results.insert(user_id, cache_entry);
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use matryx_surrealdb::test_utils::create_test_db_async;
    use matryx_surrealdb::repository::PerformanceRepository;

    #[tokio::test]
    async fn test_cache_hit_miss_tracking() {
        let db = create_test_db_async().await.expect("Failed to create test db");
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        let mut cache_manager = DeviceCacheManager::new(10, 60, performance_repo);

        // First access should be a miss
        let result = cache_manager.get_device_list("@test:example.com").await;
        assert!(result.is_ok());

        let stats = cache_manager.get_cache_stats().await;
        assert_eq!(stats.miss_count, 1);
        assert_eq!(stats.hit_count, 0);

        // Second access should be a hit
        let result = cache_manager.get_device_list("@test:example.com").await;
        assert!(result.is_ok());

        let stats = cache_manager.get_cache_stats().await;
        assert_eq!(stats.miss_count, 1);
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.hit_ratio, 0.5);
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let db = create_test_db_async().await.expect("Failed to create test db");
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        let mut cache_manager = DeviceCacheManager::new(2, 60, performance_repo); // Very small cache

        // Fill cache
        let _ = cache_manager.get_device_list("@user1:example.com").await;
        let _ = cache_manager.get_device_list("@user2:example.com").await;
        assert_eq!(cache_manager.cache.len(), 2);

        // Add third entry, should evict oldest
        let _ = cache_manager.get_device_list("@user3:example.com").await;
        assert_eq!(cache_manager.cache.len(), 2);
    }

    #[tokio::test]
    async fn test_manual_invalidation() {
        let db = create_test_db_async().await.expect("Failed to create test db");
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        let mut cache_manager = DeviceCacheManager::new(10, 60, performance_repo);

        let _ = cache_manager.get_device_list("@test:example.com").await;
        assert!(cache_manager.cache.contains_key("@test:example.com"));

        cache_manager.invalidate_user_cache("@test:example.com");
        assert!(!cache_manager.cache.contains_key("@test:example.com"));
    }

    #[tokio::test]
    async fn test_batch_fetcher() {
        let db = create_test_db_async().await.expect("Failed to create test db");
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        let cache_manager = DeviceCacheManager::new(10, 60, performance_repo);
        let mut batch_fetcher = BatchDeviceFetcher::new(cache_manager);

        let user_ids = vec![
            "@user1:example.com".to_string(),
            "@user2:example.com".to_string(),
            "@user3:example.com".to_string(),
        ];

        let result = batch_fetcher.fetch_device_lists_batch(user_ids.clone()).await;
        assert!(result.is_ok());

        let device_lists = result.expect("Batch fetch should succeed");
        assert_eq!(device_lists.len(), 3);

        for user_id in user_ids {
            assert!(device_lists.contains_key(&user_id));
        }
    }
}
