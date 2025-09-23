use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};
use matryx_surrealdb::repository::{MetricsRepository, PerformanceRepository, MonitoringRepository, HealthStatus};
use surrealdb::engine::any::Any;

/// Device-related metrics for monitoring and observability
#[derive(Debug, Clone)]
pub struct DeviceMetrics {
    pub device_registrations_total: u64,
    pub device_updates_total: u64,
    pub device_deletions_total: u64,
    pub federation_sync_operations: u64,
    pub federation_sync_failures: u64,
    pub cache_hit_total: u64,
    pub cache_miss_total: u64,
    pub average_sync_duration_ms: f64,
    pub active_device_count: u64,
    pub verification_operations_total: u64,
}

impl Default for DeviceMetrics {
    fn default() -> Self {
        Self {
            device_registrations_total: 0,
            device_updates_total: 0,
            device_deletions_total: 0,
            federation_sync_operations: 0,
            federation_sync_failures: 0,
            cache_hit_total: 0,
            cache_miss_total: 0,
            average_sync_duration_ms: 0.0,
            active_device_count: 0,
            verification_operations_total: 0,
        }
    }
}

/// Metrics collector for device management operations
pub struct DeviceMetricsCollector {
    metrics_repo: Arc<MetricsRepository<Any>>,
    performance_repo: Arc<PerformanceRepository<Any>>,
    monitoring_repo: Arc<MonitoringRepository<Any>>,
}

impl DeviceMetricsCollector {
    pub fn new(
        metrics_repo: Arc<MetricsRepository<Any>>,
        performance_repo: Arc<PerformanceRepository<Any>>,
        monitoring_repo: Arc<MonitoringRepository<Any>>,
    ) -> Self {
        Self {
            metrics_repo,
            performance_repo,
            monitoring_repo,
        }
    }

    /// Record a device registration
    pub async fn record_device_registration(&self) {
        if let Err(e) = self.metrics_repo.record_counter("matrix_device_registrations_total", 1.0, &HashMap::new()).await {
            warn!("Failed to record device registration: {}", e);
        }
        
        if let Err(e) = self.monitoring_repo.record_health_check("device_management", HealthStatus::Healthy, Some("Device registered")).await {
            warn!("Failed to record health check: {}", e);
        }
        
        info!("Device registration recorded");
    }

    /// Record a device update
    pub async fn record_device_update(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.device_updates_total += 1;
    }

    /// Record a device deletion
    pub async fn record_device_deletion(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.device_deletions_total += 1;
    }

    /// Record a federation sync operation
    pub async fn record_federation_sync(&self, duration: Duration, success: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.federation_sync_operations += 1;

        if !success {
            metrics.federation_sync_failures += 1;
            warn!(
                "Federation sync failure recorded - total failures: {}",
                metrics.federation_sync_failures
            );
        }

        // Update duration tracking
        let duration_ms = duration.as_millis() as f64;
        let mut durations = self.sync_durations.write().await;
        durations.push(duration_ms);

        // Keep only recent samples
        if durations.len() > self.max_duration_samples {
            durations.remove(0);
        }

        // Calculate average
        if !durations.is_empty() {
            metrics.average_sync_duration_ms =
                durations.iter().sum::<f64>() / durations.len() as f64;
        }
    }

    /// Record cache hit
    pub async fn record_cache_hit(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_hit_total += 1;
    }

    /// Record cache miss
    pub async fn record_cache_miss(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.cache_miss_total += 1;
    }

    /// Update active device count
    pub async fn update_active_device_count(&self, count: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.active_device_count = count;
    }

    /// Record a device verification operation
    pub async fn record_verification_operation(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.verification_operations_total += 1;
    }

    /// Get current metrics snapshot
    pub async fn get_metrics(&self) -> DeviceMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Get cache hit ratio
    pub async fn get_cache_hit_ratio(&self) -> f64 {
        let metrics = self.metrics.read().await;
        let total_requests = metrics.cache_hit_total + metrics.cache_miss_total;
        if total_requests > 0 {
            metrics.cache_hit_total as f64 / total_requests as f64
        } else {
            0.0
        }
    }

    /// Get federation success ratio
    pub async fn get_federation_success_ratio(&self) -> f64 {
        let metrics = self.metrics.read().await;
        if metrics.federation_sync_operations > 0 {
            let successful_ops =
                metrics.federation_sync_operations - metrics.federation_sync_failures;
            successful_ops as f64 / metrics.federation_sync_operations as f64
        } else {
            0.0
        }
    }

    /// Reset all metrics (useful for testing)
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = DeviceMetrics::default();

        let mut durations = self.sync_durations.write().await;
        durations.clear();

        info!("Device metrics reset");
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus_metrics(&self) -> String {
        let metrics = self.metrics.read().await;
        let cache_hit_ratio = self.get_cache_hit_ratio().await;
        let federation_success_ratio = self.get_federation_success_ratio().await;

        format!(
            r#"# HELP matrix_device_registrations_total Total number of device registrations
# TYPE matrix_device_registrations_total counter
matrix_device_registrations_total {}

# HELP matrix_device_updates_total Total number of device updates
# TYPE matrix_device_updates_total counter
matrix_device_updates_total {}

# HELP matrix_device_deletions_total Total number of device deletions
# TYPE matrix_device_deletions_total counter
matrix_device_deletions_total {}

# HELP matrix_federation_sync_operations_total Total number of federation sync operations
# TYPE matrix_federation_sync_operations_total counter
matrix_federation_sync_operations_total {}

# HELP matrix_federation_sync_failures_total Total number of federation sync failures
# TYPE matrix_federation_sync_failures_total counter
matrix_federation_sync_failures_total {}

# HELP matrix_cache_hit_total Total number of cache hits
# TYPE matrix_cache_hit_total counter
matrix_cache_hit_total {}

# HELP matrix_cache_miss_total Total number of cache misses
# TYPE matrix_cache_miss_total counter
matrix_cache_miss_total {}

# HELP matrix_cache_hit_ratio Cache hit ratio
# TYPE matrix_cache_hit_ratio gauge
matrix_cache_hit_ratio {}

# HELP matrix_federation_success_ratio Federation operation success ratio
# TYPE matrix_federation_success_ratio gauge
matrix_federation_success_ratio {}

# HELP matrix_device_sync_duration_ms Average device sync duration in milliseconds
# TYPE matrix_device_sync_duration_ms gauge
matrix_device_sync_duration_ms {}

# HELP matrix_active_devices Active device count
# TYPE matrix_active_devices gauge
matrix_active_devices {}

# HELP matrix_verification_operations_total Total number of verification operations
# TYPE matrix_verification_operations_total counter
matrix_verification_operations_total {}
"#,
            metrics.device_registrations_total,
            metrics.device_updates_total,
            metrics.device_deletions_total,
            metrics.federation_sync_operations,
            metrics.federation_sync_failures,
            metrics.cache_hit_total,
            metrics.cache_miss_total,
            cache_hit_ratio,
            federation_success_ratio,
            metrics.average_sync_duration_ms,
            metrics.active_device_count,
            metrics.verification_operations_total,
        )
    }
}

impl Default for DeviceMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics aggregator for multiple collectors
pub struct DeviceMetricsAggregator {
    collectors: HashMap<String, Arc<DeviceMetricsCollector>>,
}

impl DeviceMetricsAggregator {
    pub fn new() -> Self {
        Self { collectors: HashMap::new() }
    }

    /// Register a metrics collector with a name
    pub fn register_collector(&mut self, name: String, collector: Arc<DeviceMetricsCollector>) {
        info!("Registered device metrics collector: {}", name);
        self.collectors.insert(name, collector);
    }

    /// Get aggregated metrics from all collectors
    pub async fn get_aggregated_metrics(&self) -> DeviceMetrics {
        let mut aggregated = DeviceMetrics::default();
        let mut total_durations = 0.0;
        let mut duration_count = 0;

        for (name, collector) in &self.collectors {
            let metrics = collector.get_metrics().await;

            aggregated.device_registrations_total += metrics.device_registrations_total;
            aggregated.device_updates_total += metrics.device_updates_total;
            aggregated.device_deletions_total += metrics.device_deletions_total;
            aggregated.federation_sync_operations += metrics.federation_sync_operations;
            aggregated.federation_sync_failures += metrics.federation_sync_failures;
            aggregated.cache_hit_total += metrics.cache_hit_total;
            aggregated.cache_miss_total += metrics.cache_miss_total;
            aggregated.active_device_count += metrics.active_device_count;
            aggregated.verification_operations_total += metrics.verification_operations_total;

            if metrics.average_sync_duration_ms > 0.0 {
                total_durations += metrics.average_sync_duration_ms;
                duration_count += 1;
            }
        }

        if duration_count > 0 {
            aggregated.average_sync_duration_ms = total_durations / duration_count as f64;
        }

        aggregated
    }

    /// Export aggregated metrics in Prometheus format
    pub async fn export_aggregated_prometheus_metrics(&self) -> String {
        let aggregated = self.get_aggregated_metrics().await;

        // Calculate ratios from aggregated data
        let cache_hit_ratio = if aggregated.cache_hit_total + aggregated.cache_miss_total > 0 {
            aggregated.cache_hit_total as f64 /
                (aggregated.cache_hit_total + aggregated.cache_miss_total) as f64
        } else {
            0.0
        };

        let federation_success_ratio = if aggregated.federation_sync_operations > 0 {
            let successful_ops =
                aggregated.federation_sync_operations - aggregated.federation_sync_failures;
            successful_ops as f64 / aggregated.federation_sync_operations as f64
        } else {
            0.0
        };

        format!(
            r#"# HELP matrix_device_registrations_total_aggregated Aggregated total device registrations
# TYPE matrix_device_registrations_total_aggregated counter
matrix_device_registrations_total_aggregated {}

# HELP matrix_cache_hit_ratio_aggregated Aggregated cache hit ratio
# TYPE matrix_cache_hit_ratio_aggregated gauge
matrix_cache_hit_ratio_aggregated {}

# HELP matrix_federation_success_ratio_aggregated Aggregated federation success ratio
# TYPE matrix_federation_success_ratio_aggregated gauge
matrix_federation_success_ratio_aggregated {}

# HELP matrix_active_devices_aggregated Aggregated active device count
# TYPE matrix_active_devices_aggregated gauge
matrix_active_devices_aggregated {}
"#,
            aggregated.device_registrations_total,
            cache_hit_ratio,
            federation_success_ratio,
            aggregated.active_device_count,
        )
    }
}

impl Default for DeviceMetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = DeviceMetricsCollector::new();

        collector.record_device_registration().await;
        collector.record_device_update().await;
        collector.record_cache_hit().await;
        collector.record_cache_miss().await;

        let metrics = collector.get_metrics().await;
        assert_eq!(metrics.device_registrations_total, 1);
        assert_eq!(metrics.device_updates_total, 1);
        assert_eq!(metrics.cache_hit_total, 1);
        assert_eq!(metrics.cache_miss_total, 1);

        let hit_ratio = collector.get_cache_hit_ratio().await;
        assert_eq!(hit_ratio, 0.5);
    }

    #[tokio::test]
    async fn test_federation_sync_tracking() {
        let collector = DeviceMetricsCollector::new();

        collector.record_federation_sync(Duration::from_millis(100), true).await;
        collector.record_federation_sync(Duration::from_millis(200), false).await;

        let metrics = collector.get_metrics().await;
        assert_eq!(metrics.federation_sync_operations, 2);
        assert_eq!(metrics.federation_sync_failures, 1);
        assert_eq!(metrics.average_sync_duration_ms, 150.0);

        let success_ratio = collector.get_federation_success_ratio().await;
        assert_eq!(success_ratio, 0.5);
    }

    #[tokio::test]
    async fn test_prometheus_export() {
        let collector = DeviceMetricsCollector::new();
        collector.record_device_registration().await;

        let prometheus_output = collector.export_prometheus_metrics().await;
        assert!(prometheus_output.contains("matrix_device_registrations_total 1"));
        assert!(prometheus_output.contains("# HELP"));
        assert!(prometheus_output.contains("# TYPE"));
    }

    #[tokio::test]
    async fn test_metrics_aggregation() {
        let mut aggregator = DeviceMetricsAggregator::new();

        let collector1 = Arc::new(DeviceMetricsCollector::new());
        let collector2 = Arc::new(DeviceMetricsCollector::new());

        collector1.record_device_registration().await;
        collector2.record_device_registration().await;
        collector2.record_device_registration().await;

        aggregator.register_collector("node1".to_string(), collector1);
        aggregator.register_collector("node2".to_string(), collector2);

        let aggregated = aggregator.get_aggregated_metrics().await;
        assert_eq!(aggregated.device_registrations_total, 3);
    }
}
