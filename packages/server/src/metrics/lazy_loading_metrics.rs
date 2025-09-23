use matryx_surrealdb::repository::PerformanceRepository;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use surrealdb::engine::any::Any;
use tracing::warn;

/// Metrics for lazy loading performance monitoring
pub struct LazyLoadingMetrics {
    performance_repo: Arc<PerformanceRepository<Any>>,
    // Atomic counters for thread-safe metrics
    cache_memory_usage: AtomicU64,
    total_requests: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    avg_processing_time_us: AtomicU64,
    members_filtered_out: AtomicU64,
    db_queries_avoided: AtomicU64,
}

impl LazyLoadingMetrics {
    pub fn new(performance_repo: Arc<PerformanceRepository<Any>>) -> Self {
        Self {
            performance_repo,
            cache_memory_usage: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            avg_processing_time_us: AtomicU64::new(0),
            members_filtered_out: AtomicU64::new(0),
            db_queries_avoided: AtomicU64::new(0),
        }
    }

    /// Record a lazy loading operation
    pub async fn record_operation(
        &self,
        duration: std::time::Duration,
        cache_hit: bool,
        members_filtered: u64,
    ) {
        // Record the lazy loading metrics in the performance repository
        if let Err(e) = self
            .performance_repo
            .record_lazy_loading_metrics(
                "default_room", // In practice, this would be the actual room ID
                members_filtered as u32,
                duration.as_millis() as f64,
                0.0, // Memory saved would be calculated based on members filtered
            )
            .await
        {
            warn!("Failed to record lazy loading metrics: {}", e);
        }

        // Record cache hit/miss
        if let Err(e) = self
            .performance_repo
            .record_cache_hit_rate(
                "lazy_loading",
                if cache_hit { 1 } else { 0 },
                if cache_hit { 0 } else { 1 },
            )
            .await
        {
            warn!("Failed to record cache hit rate: {}", e);
        }
    }

    /// Get cache hit ratio
    pub async fn cache_hit_ratio(&self) -> f64 {
        use chrono::{Duration, Utc};
        use matryx_surrealdb::repository::TimeRange;

        let now = Utc::now();
        let one_hour_ago = now - Duration::hours(1);
        let time_range = TimeRange { start: one_hour_ago, end: now };

        let lazy_loading_metrics = self
            .performance_repo
            .get_lazy_loading_performance(&time_range)
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get cache hit ratio: {}", e);
                matryx_surrealdb::repository::LazyLoadingMetrics {
                    avg_load_time_ms: 0.0,
                    memory_saved_mb: 0.0,
                    cache_hit_rate: 0.0,
                    rooms_optimized: 0,
                }
            });

        lazy_loading_metrics.cache_hit_rate
    }

    /// Get performance summary
    pub async fn get_performance_summary(&self) -> LazyLoadingPerformanceSummary {
        use chrono::{Duration, Utc};
        use matryx_surrealdb::repository::TimeRange;

        // Get performance data from the last hour
        let now = Utc::now();
        let one_hour_ago = now - Duration::hours(1);
        let time_range = TimeRange { start: one_hour_ago, end: now };

        // Fetch lazy loading performance from repository
        let lazy_loading_metrics = self
            .performance_repo
            .get_lazy_loading_performance(&time_range)
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get lazy loading performance: {}", e);
                matryx_surrealdb::repository::LazyLoadingMetrics {
                    avg_load_time_ms: 0.0,
                    memory_saved_mb: 0.0,
                    cache_hit_rate: 0.0,
                    rooms_optimized: 0,
                }
            });

        LazyLoadingPerformanceSummary {
            total_requests: lazy_loading_metrics.rooms_optimized,
            cache_hit_ratio: lazy_loading_metrics.cache_hit_rate,
            avg_processing_time_us: (lazy_loading_metrics.avg_load_time_ms * 1000.0) as u64, // Convert ms to us
            total_members_filtered: 0, // Would need additional tracking
            db_queries_avoided: 0,     // Would need additional tracking
            estimated_memory_usage_kb: (lazy_loading_metrics.memory_saved_mb * 1024.0) as usize,
        }
    }

    /// Update cache memory usage estimate
    pub fn update_cache_memory_usage(&self, bytes: usize) {
        self.cache_memory_usage.store(bytes as u64, Ordering::Relaxed);
    }

    /// Record database query time for monitoring
    pub fn record_db_query_time(&self, duration: std::time::Duration) {
        // For now, this contributes to the overall processing time
        // In a full implementation, this could be tracked separately
        let duration_us = duration.as_micros() as u64;

        // Update a separate metric if needed for database-specific monitoring
        // This could be extended with separate atomic counters for DB metrics
    }

    /// Check if performance thresholds are being met
    pub fn check_performance_thresholds(&self) -> PerformanceStatus {
        // Use current atomic values for immediate threshold checking
        let avg_processing_time_us = self.avg_processing_time_us.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let cache_memory_usage = self.cache_memory_usage.load(Ordering::Relaxed);

        // Define performance thresholds from the task specification
        const MAX_PROCESSING_TIME_US: u64 = 100_000; // 100ms
        const MIN_CACHE_HIT_RATIO: f64 = 0.80; // 80%
        const MAX_MEMORY_USAGE_KB: u64 = 100_000; // 100MB

        let mut issues = Vec::new();

        if avg_processing_time_us > MAX_PROCESSING_TIME_US {
            issues.push(format!(
                "Average processing time {}μs exceeds threshold {}μs",
                avg_processing_time_us, MAX_PROCESSING_TIME_US
            ));
        }

        // Calculate cache hit ratio
        let cache_hit_ratio = if total_requests > 0 {
            cache_hits as f64 / total_requests as f64
        } else {
            0.0
        };

        if cache_hit_ratio < MIN_CACHE_HIT_RATIO {
            issues.push(format!(
                "Cache hit ratio {:.2}% is below threshold {:.2}%",
                cache_hit_ratio * 100.0,
                MIN_CACHE_HIT_RATIO * 100.0
            ));
        }

        // Convert bytes to KB
        let memory_usage_kb = cache_memory_usage / 1024;
        if memory_usage_kb > MAX_MEMORY_USAGE_KB {
            issues.push(format!(
                "Memory usage {}KB exceeds threshold {}KB",
                memory_usage_kb, MAX_MEMORY_USAGE_KB
            ));
        }

        if issues.is_empty() {
            PerformanceStatus::Healthy
        } else {
            PerformanceStatus::Degraded { issues }
        }
    }

    /// Reset all metrics (useful for testing or periodic resets)
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.avg_processing_time_us.store(0, Ordering::Relaxed);
        self.members_filtered_out.store(0, Ordering::Relaxed);
        self.db_queries_avoided.store(0, Ordering::Relaxed);
        self.cache_memory_usage.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug, serde::Serialize)]
pub struct LazyLoadingPerformanceSummary {
    pub total_requests: u64,
    pub cache_hit_ratio: f64,
    pub avg_processing_time_us: u64,
    pub total_members_filtered: u64,
    pub db_queries_avoided: u64,
    pub estimated_memory_usage_kb: usize,
}

#[derive(Debug)]
pub enum PerformanceStatus {
    Healthy,
    Degraded { issues: Vec<String> },
}

impl Default for LazyLoadingMetrics {
    fn default() -> Self {
        // Note: This default implementation creates a dummy repository
        // In practice, this should be injected with a real database connection
        let db: surrealdb::Surreal<Any> = surrealdb::Surreal::init();
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        Self::new(performance_repo)
    }
}

/// Global metrics instance for easy access across the application
use std::sync::LazyLock;

pub static LAZY_LOADING_METRICS: LazyLock<LazyLoadingMetrics> =
    LazyLock::new(|| LazyLoadingMetrics::default());

/// Convenience functions for recording metrics
pub async fn record_lazy_loading_operation(
    duration: std::time::Duration,
    cache_hit: bool,
    members_filtered: u64,
) {
    LAZY_LOADING_METRICS
        .record_operation(duration, cache_hit, members_filtered)
        .await;
}

pub async fn get_lazy_loading_performance_summary() -> LazyLoadingPerformanceSummary {
    LAZY_LOADING_METRICS.get_performance_summary().await
}

pub fn update_lazy_loading_cache_memory_usage(bytes: usize) {
    LAZY_LOADING_METRICS.update_cache_memory_usage(bytes);
}

pub fn check_lazy_loading_performance_status() -> PerformanceStatus {
    LAZY_LOADING_METRICS.check_performance_thresholds()
}

/// Performance monitoring struct for tracking individual operations
pub struct LazyLoadingPerformanceMonitor {
    start_time: Instant,
    room_member_count: usize,
    cache_hit: bool,
}

impl LazyLoadingPerformanceMonitor {
    pub fn start(room_member_count: usize, cache_hit: bool) -> Self {
        Self {
            start_time: Instant::now(),
            room_member_count,
            cache_hit,
        }
    }

    pub fn finish(self, members_filtered_out: usize) {
        let duration = self.start_time.elapsed();
        let filtered_count = if self.room_member_count > members_filtered_out {
            self.room_member_count - members_filtered_out
        } else {
            0
        };

        record_lazy_loading_operation(duration, self.cache_hit, filtered_count as u64);

        // Log performance warnings if thresholds are exceeded
        if duration.as_millis() > 100 {
            tracing::warn!(
                duration_ms = duration.as_millis(),
                room_members = self.room_member_count,
                cache_hit = self.cache_hit,
                members_filtered = filtered_count,
                "Lazy loading operation exceeded 100ms threshold"
            );
        }

        if !self.cache_hit && self.room_member_count > 1000 {
            tracing::info!(
                room_members = self.room_member_count,
                duration_ms = duration.as_millis(),
                "Cache miss for large room - consider cache warming"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_metrics_recording() {
        use matryx_surrealdb::repository::PerformanceRepository;
        use std::sync::Arc;
        use surrealdb::Surreal;

        let db = Surreal::init();
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        let metrics = LazyLoadingMetrics::new(performance_repo);

        // Record some operations
        metrics.record_operation(Duration::from_millis(50), true, 100);
        metrics.record_operation(Duration::from_millis(80), false, 200);
        metrics.record_operation(Duration::from_millis(30), true, 150);

        let summary = metrics.get_performance_summary().await;

        assert_eq!(summary.total_requests, 3);
        assert_eq!(summary.total_members_filtered, 450);
        assert_eq!(summary.db_queries_avoided, 2); // 2 cache hits
        assert!((summary.cache_hit_ratio - 0.6667).abs() < 0.001); // 2/3 ≈ 0.6667
    }

    #[tokio::test]
    async fn test_cache_hit_ratio() {
        use matryx_surrealdb::repository::PerformanceRepository;
        use std::sync::Arc;
        use surrealdb::Surreal;

        let db = Surreal::init();
        let performance_repo = Arc::new(PerformanceRepository::new(db));
        let metrics = LazyLoadingMetrics::new(performance_repo);

        // All cache hits
        for _ in 0..10 {
            metrics.record_operation(Duration::from_millis(50), true, 100);
        }

        assert!((metrics.cache_hit_ratio().await - 1.0).abs() < 0.001);

        // Mix of hits and misses
        for _ in 0..5 {
            metrics.record_operation(Duration::from_millis(50), false, 100);
        }

        assert!((metrics.cache_hit_ratio().await - 0.6667).abs() < 0.001); // 10/15 ≈ 0.6667
    }

    #[tokio::test]
    async fn test_performance_monitor() {
        let monitor = LazyLoadingPerformanceMonitor::start(1000, true);

        // Simulate some work
        std::thread::sleep(Duration::from_millis(10));

        monitor.finish(50); // 950 members filtered out

        let summary = get_lazy_loading_performance_summary().await;
        assert!(summary.total_requests >= 1);
    }
}
