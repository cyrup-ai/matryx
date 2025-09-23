use matryx_surrealdb::repository::{HealthStatus, MonitoringRepository, PerformanceRepository};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use surrealdb::engine::any::Any;
use tracing::warn;

/// Memory tracker for lazy loading cache lifecycle management
pub struct LazyLoadingMemoryTracker {
    performance_repo: Arc<PerformanceRepository<Any>>,
    monitoring_repo: Arc<MonitoringRepository<Any>>,
    baseline_memory: AtomicUsize,
    current_memory: AtomicUsize,
    peak_memory: AtomicUsize,
    measurement_count: AtomicU64,
    last_measurement: Mutex<Option<Instant>>,
}

impl LazyLoadingMemoryTracker {
    pub fn new(
        performance_repo: Arc<PerformanceRepository<Any>>,
        monitoring_repo: Arc<MonitoringRepository<Any>>,
    ) -> Self {
        Self {
            performance_repo,
            monitoring_repo,
            baseline_memory: AtomicUsize::new(0),
            current_memory: AtomicUsize::new(0),
            peak_memory: AtomicUsize::new(0),
            measurement_count: AtomicU64::new(0),
            last_measurement: Mutex::new(None),
        }
    }

    /// Set the baseline memory usage
    pub fn set_baseline(&self, bytes: usize) {
        self.baseline_memory.store(bytes, Ordering::Relaxed);
        self.current_memory.store(bytes, Ordering::Relaxed);
        self.peak_memory.store(bytes, Ordering::Relaxed);
    }

    /// Update current memory usage
    pub async fn update_memory_usage(&self, bytes: usize) {
        // Update atomic counters
        self.current_memory.store(bytes, Ordering::Relaxed);
        self.measurement_count.fetch_add(1, Ordering::Relaxed);

        // Update peak if necessary
        let current_peak = self.peak_memory.load(Ordering::Relaxed);
        if bytes > current_peak {
            self.peak_memory.store(bytes, Ordering::Relaxed);
        }

        // Update last measurement time
        if let Ok(mut last) = self.last_measurement.lock() {
            *last = Some(Instant::now());
        }

        let memory_mb = bytes as f64 / (1024.0 * 1024.0);

        // Record memory usage in performance repository
        if let Err(e) = self.performance_repo.record_memory_usage("lazy_loading", memory_mb).await {
            warn!("Failed to record memory usage: {}", e);
        }

        // Check if memory usage is concerning and create health check
        let health_status = if memory_mb > 100.0 {
            HealthStatus::Unhealthy
        } else if memory_mb > 75.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        if let Err(e) = self
            .monitoring_repo
            .record_health_check(
                "memory_tracker",
                health_status,
                Some(&format!("Memory usage: {:.2} MB", memory_mb)),
            )
            .await
        {
            warn!("Failed to record health check: {}", e);
        }
    }

    /// Get current memory statistics
    pub async fn get_memory_stats(&self) -> MemoryStats {
        let baseline_mb = self.baseline_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        let current_mb = self.current_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        let peak_mb = self.peak_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);

        let health_status = self.assess_memory_health(current_mb, peak_mb);

        MemoryStats {
            baseline_memory_mb: baseline_mb,
            current_memory_mb: current_mb,
            peak_memory_mb: peak_mb,
            memory_growth_ratio: if baseline_mb > 0.0 {
                current_mb / baseline_mb
            } else {
                1.0
            },
            measurement_count: self.measurement_count.load(Ordering::Relaxed),
            health_status,
        }
    }

    /// Assess memory health status
    fn assess_memory_health(&self, current_mb: f64, peak_mb: f64) -> MemoryHealthStatus {
        const WARNING_THRESHOLD_MB: f64 = 75.0; // 75MB warning
        const CRITICAL_THRESHOLD_MB: f64 = 100.0; // 100MB critical
        const GROWTH_WARNING_RATIO: f64 = 3.0; // 3x growth warning

        let baseline_mb = self.baseline_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        let growth_ratio = if baseline_mb > 0.0 {
            current_mb / baseline_mb
        } else {
            1.0
        };

        if current_mb >= CRITICAL_THRESHOLD_MB || growth_ratio >= GROWTH_WARNING_RATIO * 2.0 {
            MemoryHealthStatus::Critical
        } else if current_mb >= WARNING_THRESHOLD_MB || growth_ratio >= GROWTH_WARNING_RATIO {
            MemoryHealthStatus::Warning
        } else {
            MemoryHealthStatus::Healthy
        }
    }

    /// Check if memory usage is within acceptable limits
    pub fn is_memory_healthy(&self) -> bool {
        let current_mb = self.current_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        let baseline_mb = self.baseline_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);

        const MAX_MEMORY_MB: f64 = 100.0;
        const MAX_GROWTH_RATIO: f64 = 5.0;

        let growth_ratio = if baseline_mb > 0.0 {
            current_mb / baseline_mb
        } else {
            1.0
        };

        current_mb < MAX_MEMORY_MB && growth_ratio < MAX_GROWTH_RATIO
    }

    /// Get memory efficiency score (0.0 to 1.0)
    pub fn get_memory_efficiency_score(&self) -> f64 {
        let current_mb = self.current_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        let baseline_mb = self.baseline_memory.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);

        if baseline_mb <= 0.0 {
            return 1.0; // Perfect score if no baseline
        }

        let growth_ratio = current_mb / baseline_mb;

        // Score decreases as memory grows beyond baseline
        // Perfect score (1.0) for no growth, decreasing score for higher growth
        const ACCEPTABLE_GROWTH: f64 = 2.0; // 2x growth is still acceptable

        if growth_ratio <= 1.0 {
            1.0 // Perfect efficiency
        } else if growth_ratio <= ACCEPTABLE_GROWTH {
            1.0 - ((growth_ratio - 1.0) / ACCEPTABLE_GROWTH) * 0.5 // 0.5 to 1.0 range
        } else {
            0.5 / growth_ratio // Rapidly decreasing efficiency for high growth
        }
    }

    /// Reset memory tracking (useful for testing or maintenance)
    pub fn reset(&self) {
        let baseline = self.baseline_memory.load(Ordering::Relaxed);
        self.current_memory.store(baseline, Ordering::Relaxed);
        self.peak_memory.store(baseline, Ordering::Relaxed);
        self.measurement_count.store(0, Ordering::Relaxed);
        if let Ok(mut last) = self.last_measurement.lock() {
            *last = None;
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct MemoryStats {
    pub baseline_memory_mb: f64,
    pub current_memory_mb: f64,
    pub peak_memory_mb: f64,
    pub memory_growth_ratio: f64,
    pub measurement_count: u64,
    pub health_status: MemoryHealthStatus,
}

#[derive(Debug, serde::Serialize)]
pub enum MemoryHealthStatus {
    Healthy,
    Warning,
    Critical,
}

impl Default for LazyLoadingMemoryTracker {
    fn default() -> Self {
        let db = surrealdb::Surreal::init();
        let performance_repo = Arc::new(PerformanceRepository::new(db.clone()));
        let monitoring_repo = Arc::new(MonitoringRepository::new(db));
        Self::new(performance_repo, monitoring_repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_tracker_basic_functionality() {
        let tracker = LazyLoadingMemoryTracker::default();

        // Set baseline
        tracker.set_baseline(1024 * 1024); // 1MB baseline

        // Update memory usage
        tracker.update_memory_usage(2 * 1024 * 1024).await; // 2MB current

        // Check if tracking works
        assert_eq!(tracker.current_memory.load(Ordering::Relaxed), 2 * 1024 * 1024);
        assert_eq!(tracker.peak_memory.load(Ordering::Relaxed), 2 * 1024 * 1024);
        assert_eq!(tracker.measurement_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_memory_health_assessment() {
        let tracker = LazyLoadingMemoryTracker::default();
        tracker.set_baseline(10 * 1024 * 1024); // 10MB baseline

        // Healthy usage
        tracker.update_memory_usage(15 * 1024 * 1024).await; // 15MB
        let stats = tracker.get_memory_stats().await;
        assert!(matches!(stats.health_status, MemoryHealthStatus::Healthy));

        // Warning level usage
        tracker.update_memory_usage(80 * 1024 * 1024).await; // 80MB
        let stats = tracker.get_memory_stats().await;
        assert!(matches!(stats.health_status, MemoryHealthStatus::Warning));

        // Critical usage
        tracker.update_memory_usage(110 * 1024 * 1024).await; // 110MB
        let stats = tracker.get_memory_stats().await;
        assert!(matches!(stats.health_status, MemoryHealthStatus::Critical));
    }

    #[tokio::test]
    async fn test_memory_efficiency_score() {
        let tracker = LazyLoadingMemoryTracker::default();
        tracker.set_baseline(10 * 1024 * 1024); // 10MB baseline

        // No growth - perfect efficiency
        tracker.update_memory_usage(10 * 1024 * 1024).await;
        assert!((tracker.get_memory_efficiency_score() - 1.0).abs() < 0.001);

        // Moderate growth - good efficiency
        tracker.update_memory_usage(15 * 1024 * 1024).await; // 1.5x growth
        let score = tracker.get_memory_efficiency_score();
        assert!(score > 0.7 && score <= 1.0);

        // High growth - poor efficiency
        tracker.update_memory_usage(50 * 1024 * 1024).await; // 5x growth
        let score = tracker.get_memory_efficiency_score();
        assert!(score < 0.3);
    }
}
