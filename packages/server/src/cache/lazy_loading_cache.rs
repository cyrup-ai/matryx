use futures::stream::StreamExt;
use matryx_entity::types::Event;
use matryx_surrealdb::repository::membership::MembershipRepository;
use moka::future::Cache;
use std::collections::HashSet;
use std::time::Duration;
use tracing::debug;

/// Configuration for LazyLoadingCache instances
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LazyLoadingCacheConfig {
    pub essential_members_ttl: Duration,
    pub essential_members_capacity: u64,
    pub power_hierarchies_ttl: Duration,
    pub power_hierarchies_capacity: u64,
    pub room_creators_ttl: Duration,
    pub room_creators_capacity: u64,
    pub membership_events_ttl: Duration,
    pub membership_events_capacity: u64,
}

/// Specialized cache for lazy loading optimization
#[derive(Clone)]
pub struct LazyLoadingCache {
    /// Cache for essential members per room
    essential_members: Cache<String, HashSet<String>>,
    /// Cache for room power level hierarchies  
    power_hierarchies: Cache<String, Vec<(String, i64)>>,
    /// Cache for room creators
    room_creators: Cache<String, Option<String>>,
    /// Cache for filtered membership events
    membership_events: Cache<String, Vec<Event>>,
}

impl LazyLoadingCache {
    pub fn new() -> Self {
        Self {
            essential_members: Cache::builder()
                .time_to_live(Duration::from_secs(300)) // 5 minutes
                .max_capacity(5000) // 5000 rooms
                .build(),
            power_hierarchies: Cache::builder()
                .time_to_live(Duration::from_secs(600)) // 10 minutes (power levels change less)
                .max_capacity(10000)
                .build(),
            room_creators: Cache::builder()
                .time_to_live(Duration::from_secs(3600)) // 1 hour (creators never change)
                .max_capacity(50000)
                .build(),
            membership_events: Cache::builder()
                .time_to_live(Duration::from_secs(120)) // 2 minutes (events change frequently)
                .max_capacity(2000)
                .build(),
        }
    }

    /// Get essential members from cache (simplified interface for migration code)
    #[allow(dead_code)]
    pub async fn get_essential_members(&self, cache_key: &str) -> Option<HashSet<String>> {
        self.essential_members.get(cache_key).await
    }

    /// Store essential members in cache (simplified interface for migration code)
    #[allow(dead_code)]
    pub async fn store_essential_members(&self, cache_key: &str, members: &HashSet<String>) {
        self.essential_members.insert(cache_key.to_string(), members.clone()).await;
        tracing::debug!(
            "Stored {} essential members in cache with key: {}",
            members.len(),
            cache_key
        );
    }

    /// Get essential members with multi-level caching
    pub async fn get_essential_members_cached(
        &self,
        room_id: &str,
        user_id: &str,
        timeline_senders: &[String],
        repo: &MembershipRepository,
    ) -> Result<HashSet<String>, Box<dyn std::error::Error + Send + Sync>> {
        let cache_key = format!("{}:{}:{}", room_id, user_id, timeline_senders.join(","));

        if let Some(cached) = self.essential_members.get(&cache_key).await {
            return Ok(cached);
        }

        // Get from database with optimization
        let memberships = repo
            .get_essential_members_optimized(room_id, user_id, timeline_senders)
            .await?;

        let essential_members: HashSet<String> =
            memberships.into_iter().map(|m| m.user_id).collect();

        self.essential_members.insert(cache_key, essential_members.clone()).await;
        Ok(essential_members)
    }

    /// Get power hierarchy with caching
    #[allow(dead_code)]
    pub async fn get_power_hierarchy_cached(
        &self,
        room_id: &str,
        repo: &MembershipRepository,
    ) -> Result<Vec<(String, i64)>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(cached) = self.power_hierarchies.get(room_id).await {
            return Ok(cached);
        }

        let hierarchy = repo.get_room_power_hierarchy(room_id).await?;
        self.power_hierarchies.insert(room_id.to_string(), hierarchy.clone()).await;

        Ok(hierarchy)
    }

    /// Get room creator with caching
    #[allow(dead_code)]
    pub async fn get_room_creator_cached(
        &self,
        room_id: &str,
        repo: &MembershipRepository,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(cached) = self.room_creators.get(room_id).await {
            return Ok(cached);
        }

        // Use the repository's cached method which has its own internal cache
        let creator = repo.get_room_creator_cached(room_id, &self.room_creators).await?;

        Ok(creator)
    }

    /// Cache filtered membership events
    pub async fn cache_membership_events(
        &self,
        room_id: &str,
        filter_hash: &str,
        events: Vec<Event>,
    ) {
        let cache_key = format!("{}:{}", room_id, filter_hash);
        self.membership_events.insert(cache_key, events).await;
    }

    /// Get cached membership events
    pub async fn get_cached_membership_events(
        &self,
        room_id: &str,
        filter_hash: &str,
    ) -> Option<Vec<Event>> {
        let cache_key = format!("{}:{}", room_id, filter_hash);
        self.membership_events.get(&cache_key).await
    }

    /// Invalidate all cache entries for a room
    pub async fn invalidate_room_cache(&self, room_id: &str) {
        let room_id_prefix = format!("{}:", room_id);

        // Invalidate essential members cache
        let _ = self
            .essential_members
            .invalidate_entries_if(move |key, _| key.starts_with(&room_id_prefix));

        // Invalidate power hierarchies cache
        self.power_hierarchies.remove(room_id).await;

        // Invalidate room creator cache
        self.room_creators.remove(room_id).await;

        // Invalidate membership events cache
        let room_id_prefix_2 = format!("{}:", room_id);
        let _ = self
            .membership_events
            .invalidate_entries_if(move |key, _| key.starts_with(&room_id_prefix_2));
    }

    /// Get cache statistics for monitoring
    pub async fn get_cache_stats(&self) -> LazyLoadingCacheStats {
        LazyLoadingCacheStats {
            essential_members_size: self.essential_members.entry_count(),
            power_hierarchies_size: self.power_hierarchies.entry_count(),
            room_creators_size: self.room_creators.entry_count(),
            membership_events_size: self.membership_events.entry_count(),
            essential_members_hit_count: 0, // Moka doesn't expose hit/miss stats
            essential_members_miss_count: 0,
            power_hierarchies_hit_count: 0,
            power_hierarchies_miss_count: 0,
            room_creators_hit_count: 0,
            room_creators_miss_count: 0,
            membership_events_hit_count: 0,
            membership_events_miss_count: 0,
        }
    }

    /// Calculate cache hit ratio for performance monitoring
    pub async fn get_cache_hit_ratio(&self) -> f64 {
        let stats = self.get_cache_stats().await;
        let total_hits = stats.essential_members_hit_count
            + stats.power_hierarchies_hit_count
            + stats.room_creators_hit_count
            + stats.membership_events_hit_count;
        let total_requests = total_hits
            + stats.essential_members_miss_count
            + stats.power_hierarchies_miss_count
            + stats.room_creators_miss_count
            + stats.membership_events_miss_count;

        if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        }
    }

    /// Warm cache for frequently accessed rooms
    #[allow(dead_code)]
    pub async fn warm_cache_for_room(
        &self,
        room_id: &str,
        user_id: &str,
        repo: &MembershipRepository,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Pre-populate power hierarchy cache
        let _ = self.get_power_hierarchy_cached(room_id, repo).await;

        // Pre-populate room creator cache
        let _ = self.get_room_creator_cached(room_id, repo).await;

        // Pre-populate essential members for common scenarios (empty timeline senders)
        let empty_senders: Vec<String> = vec![];
        let _ = self
            .get_essential_members_cached(room_id, user_id, &empty_senders, repo)
            .await;

        Ok(())
    }

    /// Batch invalidate multiple rooms (for efficient bulk operations)
    #[allow(dead_code)]
    pub async fn batch_invalidate_rooms(&self, room_ids: &[String]) {
        for room_id in room_ids {
            self.invalidate_room_cache(room_id).await;
        }
    }

    /// Start live invalidation for a room using SurrealDB LiveQuery
    /// This integrates with the membership repository to receive real-time updates
    pub async fn start_live_invalidation(
        &self,
        room_id: &str,
        repo: &MembershipRepository,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create a channel for cache invalidation signals
        let (invalidation_tx, _invalidation_rx) = tokio::sync::broadcast::channel(100);

        // Start the enhanced membership subscription with cache invalidation
        let membership_stream = repo
            .subscribe_room_membership_enhanced(room_id, invalidation_tx.clone())
            .await?;

        // Start the enhanced power levels subscription
        let power_levels_stream =
            repo.subscribe_power_levels_enhanced(room_id, invalidation_tx).await?;

        // Spawn background tasks to manage the streams and handle cache invalidation
        let room_id_clone = room_id.to_string();
        let cache_clone = self.clone();
        tokio::spawn(async move {
            debug!("Starting membership stream processing for room {}", room_id_clone);
            
            let mut stream = membership_stream;
            while let Some(memberships) = stream.next().await {
                if !memberships.is_empty() {
                    tracing::debug!(
                        room_id = %room_id_clone,
                        count = memberships.len(),
                        "Processing membership changes for cache invalidation"
                    );
                    
                    // Invalidate cache for this room
                    cache_clone.invalidate_room_cache(&room_id_clone).await;
                }
            }
            
            tracing::warn!(
                room_id = %room_id_clone,
                "Membership stream terminated"
            );
        });

        let room_id_clone2 = room_id.to_string();
        let cache_clone2 = self.clone();
        tokio::spawn(async move {
            debug!("Starting power levels stream processing for room {}", room_id_clone2);
            
            let mut stream = power_levels_stream;
            while let Some(power_levels) = stream.next().await {
                if !power_levels.is_empty() {
                    tracing::debug!(
                        room_id = %room_id_clone2,
                        count = power_levels.len(),
                        "Processing power level changes for cache invalidation"
                    );
                    
                    // Invalidate cache for this room
                    cache_clone2.invalidate_room_cache(&room_id_clone2).await;
                }
            }
            
            tracing::warn!(
                room_id = %room_id_clone2,
                "Power levels stream terminated"
            );
        });

        tracing::debug!(
            room_id = %room_id,
            "Started live cache invalidation streams for room"
        );

        Ok(())
    }

    /// Get estimated memory usage across all caches
    pub async fn get_estimated_memory_usage_bytes(&self) -> usize {
        let stats = self.get_cache_stats().await;

        // Rough estimates based on typical data sizes
        let essential_members_memory = stats.essential_members_size as usize * 256; // ~256 bytes per entry
        let power_hierarchies_memory = stats.power_hierarchies_size as usize * 128; // ~128 bytes per entry
        let room_creators_memory = stats.room_creators_size as usize * 64; // ~64 bytes per entry
        let membership_events_memory = stats.membership_events_size as usize * 1024; // ~1KB per event set

        essential_members_memory
            + power_hierarchies_memory
            + room_creators_memory
            + membership_events_memory
    }
}

#[derive(Debug)]
pub struct LazyLoadingCacheStats {
    pub essential_members_size: u64,
    pub power_hierarchies_size: u64,
    pub room_creators_size: u64,
    pub membership_events_size: u64,
    pub essential_members_hit_count: u64,
    pub essential_members_miss_count: u64,
    pub power_hierarchies_hit_count: u64,
    pub power_hierarchies_miss_count: u64,
    pub room_creators_hit_count: u64,
    pub room_creators_miss_count: u64,
    pub membership_events_hit_count: u64,
    pub membership_events_miss_count: u64,
}

impl Default for LazyLoadingCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration-driven cache creation for production tuning
impl LazyLoadingCache {
    #[allow(dead_code)]
    pub fn with_config(config: LazyLoadingCacheConfig) -> Self {
        Self {
            essential_members: Cache::builder()
                .time_to_live(config.essential_members_ttl)
                .max_capacity(config.essential_members_capacity)
                .build(),
            power_hierarchies: Cache::builder()
                .time_to_live(config.power_hierarchies_ttl)
                .max_capacity(config.power_hierarchies_capacity)
                .build(),
            room_creators: Cache::builder()
                .time_to_live(config.room_creators_ttl)
                .max_capacity(config.room_creators_capacity)
                .build(),
            membership_events: Cache::builder()
                .time_to_live(config.membership_events_ttl)
                .max_capacity(config.membership_events_capacity)
                .build(),
        }
    }

    #[allow(dead_code)]
    pub fn production_config() -> LazyLoadingCacheConfig {
        LazyLoadingCacheConfig {
            essential_members_ttl: Duration::from_secs(180),
            essential_members_capacity: 10000,
            power_hierarchies_ttl: Duration::from_secs(600),
            power_hierarchies_capacity: 20000,
            room_creators_ttl: Duration::from_secs(1800),
            room_creators_capacity: 100000,
            membership_events_ttl: Duration::from_secs(60),
            membership_events_capacity: 5000,
        }
    }

    #[allow(dead_code)]
    pub fn development_config() -> LazyLoadingCacheConfig {
        LazyLoadingCacheConfig {
            essential_members_ttl: Duration::from_secs(60),
            essential_members_capacity: 1000,
            power_hierarchies_ttl: Duration::from_secs(120),
            power_hierarchies_capacity: 2000,
            room_creators_ttl: Duration::from_secs(300),
            room_creators_capacity: 10000,
            membership_events_ttl: Duration::from_secs(30),
            membership_events_capacity: 500,
        }
    }

    /// Graceful shutdown of cache components
    #[allow(dead_code)]
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Moka caches don't require explicit shutdown, but we can provide cleanup logging
        tracing::info!("Shutting down lazy loading cache system");

        // Get final statistics before shutdown
        let final_stats = self.get_cache_stats().await;
        tracing::info!(
            essential_members_entries = final_stats.essential_members_size,
            power_hierarchies_entries = final_stats.power_hierarchies_size,
            room_creators_entries = final_stats.room_creators_size,
            membership_events_entries = final_stats.membership_events_size,
            "Final cache statistics before shutdown"
        );

        // Moka caches will be dropped and cleaned up automatically
        Ok(())
    }

    /// Health check for cache system
    pub async fn health_check(&self) -> LazyLoadingHealthStatus {
        let stats = self.get_cache_stats().await;
        let hit_ratio = self.get_cache_hit_ratio().await;
        let memory_usage = self.get_estimated_memory_usage_bytes().await;

        // Define health thresholds
        const HEALTHY_HIT_RATIO: f64 = 0.70; // 70% minimum
        const MAX_MEMORY_BYTES: usize = 100 * 1024 * 1024; // 100MB

        let is_healthy = hit_ratio >= HEALTHY_HIT_RATIO && memory_usage <= MAX_MEMORY_BYTES;

        let status = if is_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        };

        let mut issues = Vec::new();
        if hit_ratio < HEALTHY_HIT_RATIO {
            issues.push(format!(
                "Cache hit ratio {:.2} below threshold {:.2}",
                hit_ratio, HEALTHY_HIT_RATIO
            ));
        }
        if memory_usage > MAX_MEMORY_BYTES {
            issues.push(format!(
                "Memory usage {}MB exceeds threshold {}MB",
                memory_usage / (1024 * 1024),
                MAX_MEMORY_BYTES / (1024 * 1024)
            ));
        }

        LazyLoadingHealthStatus {
            status,
            cache_hit_ratio: hit_ratio,
            memory_usage_bytes: memory_usage,
            total_entries: stats.essential_members_size
                + stats.power_hierarchies_size
                + stats.room_creators_size
                + stats.membership_events_size,
            issues,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct LazyLoadingHealthStatus {
    pub status: String,
    pub cache_hit_ratio: f64,
    pub memory_usage_bytes: usize,
    pub total_entries: u64,
    pub issues: Vec<String>,
}

impl LazyLoadingHealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.status == "healthy"
    }
}
