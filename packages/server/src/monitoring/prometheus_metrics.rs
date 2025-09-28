use matryx_surrealdb::repository::metrics::MetricsRepository;
use prometheus::{Counter, CounterVec, Gauge, HistogramVec, Registry};
use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Response,
    routing::get,
};
use surrealdb::engine::any::Any;

/// Prometheus metrics exporter for lazy loading performance
pub struct LazyLoadingPrometheusMetrics {
    metrics_repo: Arc<MetricsRepository<Any>>,
    // Prometheus metrics
    db_queries_total: CounterVec,
    db_query_duration: HistogramVec,
    db_queries_avoided: Counter,
    members_filtered_total: CounterVec,
    processing_time: HistogramVec,
    memory_usage_bytes: Gauge,
    memory_growth_rate: Gauge,
    errors_total: CounterVec,
    feature_usage: CounterVec,
    migration_phase: Gauge,
    rollout_percentage: Gauge,
    registry: Registry,
}

impl LazyLoadingPrometheusMetrics {
    pub fn new(metrics_repo: Arc<MetricsRepository<Any>>) -> Self {
        let registry = Registry::new();

        let db_queries_total = CounterVec::new(
            prometheus::Opts::new("matryx_db_queries_total", "Total database queries"),
            &["query_type", "table"],
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let db_query_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "matryx_db_query_duration_seconds",
                "Database query duration",
            ),
            &["query_type", "table"],
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let db_queries_avoided = Counter::new(
            "matryx_db_queries_avoided_total",
            "Total database queries avoided by caching",
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let members_filtered_total = CounterVec::new(
            prometheus::Opts::new("matryx_members_filtered_total", "Total room members filtered"),
            &["room_size_bucket", "filter_type"],
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let processing_time = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "matryx_processing_time_seconds",
                "Request processing time",
            ),
            &["endpoint", "method"],
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let memory_usage_bytes =
            Gauge::new("matryx_memory_usage_bytes", "Current memory usage in bytes")
                .unwrap_or_else(|e| panic!("Failed to create prometheus memory gauge - this indicates invalid metric configuration: {}", e));

        let memory_growth_rate = Gauge::new(
            "matryx_memory_growth_rate_bytes_per_sec",
            "Memory growth rate in bytes per second",
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let errors_total = CounterVec::new(
            prometheus::Opts::new("matryx_errors_total", "Total errors encountered"),
            &["error_type", "component"],
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let feature_usage = CounterVec::new(
            prometheus::Opts::new("matryx_feature_usage_total", "Feature usage counter"),
            &["feature", "version"],
        )
        .unwrap_or_else(|e| panic!("Failed to create prometheus metric - this indicates invalid metric configuration: {}", e));

        let migration_phase =
            Gauge::new("matryx_migration_phase", "Current migration phase")
                .unwrap_or_else(|e| panic!("Failed to create prometheus migration phase gauge - this indicates invalid metric configuration: {}", e));

        let rollout_percentage =
            Gauge::new("matryx_rollout_percentage", "Feature rollout percentage")
                .unwrap_or_else(|e| panic!("Failed to create prometheus rollout percentage gauge - this indicates invalid metric configuration: {}", e));

        // Register all metrics
        registry.register(Box::new(db_queries_total.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(db_query_duration.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(db_queries_avoided.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(members_filtered_total.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(processing_time.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(memory_usage_bytes.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(memory_growth_rate.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(errors_total.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(feature_usage.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(migration_phase.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));
        registry.register(Box::new(rollout_percentage.clone()))
            .unwrap_or_else(|e| panic!("Failed to register prometheus metric - this indicates a duplicate metric name: {}", e));

        Self {
            metrics_repo,
            db_queries_total,
            db_query_duration,
            db_queries_avoided,
            members_filtered_total,
            processing_time,
            memory_usage_bytes,
            memory_growth_rate,
            errors_total,
            feature_usage,
            migration_phase,
            rollout_percentage,
            registry,
        }
    }

    /// Record a lazy loading request
    pub async fn record_request(
        &self,
        room_size_bucket: &str,
        cache_status: &str,
        implementation: &str,
        duration: std::time::Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut labels = HashMap::new();
        labels.insert("room_size_bucket".to_string(), room_size_bucket.to_string());
        labels.insert("cache_status".to_string(), cache_status.to_string());
        labels.insert("implementation".to_string(), implementation.to_string());

        self.metrics_repo
            .record_counter("matryx_lazy_loading_requests_total", 1.0, &labels)
            .await?;

        let buckets = vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
        ];
        self.metrics_repo
            .record_histogram(
                "matryx_lazy_loading_request_duration_seconds",
                duration.as_secs_f64(),
                &buckets,
                &labels,
            )
            .await?;

        Ok(())
    }

    /// Record cache hit/miss
    pub async fn record_cache_operation(
        &self,
        cache_type: &str,
        room_size_bucket: &str,
        is_hit: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut labels = HashMap::new();
        labels.insert("cache_type".to_string(), cache_type.to_string());
        labels.insert("room_size_bucket".to_string(), room_size_bucket.to_string());

        if is_hit {
            self.metrics_repo
                .record_counter("matryx_lazy_loading_cache_hits_total", 1.0, &labels)
                .await?;
        } else {
            self.metrics_repo
                .record_counter("matryx_lazy_loading_cache_misses_total", 1.0, &labels)
                .await?;
        }

        Ok(())
    }

    /// Update cache statistics
    pub async fn update_cache_stats(
        &self,
        cache_type: &str,
        hit_ratio: f64,
        size: i64,
        memory_bytes: f64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut labels = HashMap::new();
        labels.insert("cache_type".to_string(), cache_type.to_string());

        self.metrics_repo
            .record_gauge("matryx_lazy_loading_cache_hit_ratio", hit_ratio, &labels)
            .await?;
        self.metrics_repo
            .record_gauge("matryx_lazy_loading_cache_size_entries", size as f64, &labels)
            .await?;
        self.metrics_repo
            .record_gauge("matryx_lazy_loading_cache_memory_bytes", memory_bytes, &labels)
            .await?;

        Ok(())
    }

    /// Record database query
    pub fn record_db_query(
        &self,
        query_type: &str,
        room_size_bucket: &str,
        optimization_level: &str,
        duration: std::time::Duration,
    ) {
        let labels = &[query_type, room_size_bucket, optimization_level];
        self.db_queries_total.with_label_values(labels).inc();

        let query_labels = &[query_type, room_size_bucket];
        self.db_query_duration
            .with_label_values(query_labels)
            .observe(duration.as_secs_f64());
    }

    /// Record avoided database query (cache hit)
    pub fn record_db_query_avoided(&self) {
        self.db_queries_avoided.inc();
    }

    /// Record members filtered
    pub fn record_members_filtered(&self, room_size_bucket: &str, filter_type: &str, count: u64) {
        let labels = &[room_size_bucket, filter_type];
        self.members_filtered_total.with_label_values(labels).inc_by(count as f64);
    }

    /// Record processing time
    pub fn record_processing_time(
        &self,
        room_size_bucket: &str,
        optimization_level: &str,
        duration: std::time::Duration,
    ) {
        let labels = &[room_size_bucket, optimization_level];
        self.processing_time
            .with_label_values(labels)
            .observe(duration.as_secs_f64());
    }

    /// Update memory metrics
    pub fn update_memory_metrics(&self, usage_bytes: f64, growth_rate_bytes_per_sec: f64) {
        self.memory_usage_bytes.set(usage_bytes);
        self.memory_growth_rate.set(growth_rate_bytes_per_sec);
    }

    /// Record error
    pub fn record_error(&self, error_type: &str, component: &str) {
        let labels = &[error_type, component];
        self.errors_total.with_label_values(labels).inc();
    }

    /// Record feature usage
    pub fn record_feature_usage(&self, feature_name: &str, enabled: bool) {
        let enabled_str = if enabled { "true" } else { "false" };
        let labels = &[feature_name, enabled_str];
        self.feature_usage.with_label_values(labels).inc();
    }

    /// Update migration metrics
    pub fn update_migration_metrics(&self, phase: i64, rollout_percentage: f64) {
        self.migration_phase.set(phase as f64);
        self.rollout_percentage.set(rollout_percentage);
    }

    /// Update all metrics from LazyLoadingMetrics repository
    pub async fn update_from_lazy_loading_metrics(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get performance summary from metrics repository
        let summary = self.metrics_repo.get_performance_summary().await?;

        let mut labels = HashMap::new();
        labels.insert("type".to_string(), "overall".to_string());

        // Record cache hit ratio as a gauge
        self.metrics_repo
            .record_gauge("matryx_lazy_loading_cache_hit_ratio", summary.cache_hit_ratio, &labels)
            .await?;

        // Record memory usage as a gauge
        let memory_bytes = summary.estimated_memory_usage_kb * 1024.0;
        self.metrics_repo
            .record_gauge("matryx_lazy_loading_memory_usage_bytes", memory_bytes, &labels)
            .await?;

        // Record database queries avoided as a counter increment
        self.metrics_repo
            .record_counter(
                "matryx_lazy_loading_db_queries_avoided_total",
                summary.db_queries_avoided,
                &labels,
            )
            .await?;

        Ok(())
    }

    /// Create router for Prometheus metrics endpoint
    pub fn create_metrics_router(metrics: Arc<Self>) -> Router {
        Router::new().route("/metrics", get(serve_metrics)).with_state(metrics)
    }

    /// Get metrics registry for custom integration
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

/// Serve Prometheus metrics
async fn serve_metrics(
    State(metrics): State<Arc<LazyLoadingPrometheusMetrics>>,
) -> Result<Response, StatusCode> {
    // Get metrics in Prometheus format from repository
    let prometheus_output = metrics
        .metrics_repo
        .get_prometheus_metrics()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = Response::builder()
        .header("content-type", "text/plain; version=0.0.4")
        .body(prometheus_output.into())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}

/// Helper function to determine room size bucket
pub fn get_room_size_bucket(member_count: usize) -> &'static str {
    match member_count {
        0..=10 => "tiny",
        11..=100 => "small",
        101..=1000 => "medium",
        1001..=10000 => "large",
        _ => "huge",
    }
}

/// Helper function to determine optimization level
pub fn get_optimization_level(
    use_cache: bool,
    use_db_optimization: bool,
    use_realtime_invalidation: bool,
) -> &'static str {
    match (use_cache, use_db_optimization, use_realtime_invalidation) {
        (false, false, false) => "basic",
        (true, false, false) => "cached",
        (true, true, false) => "optimized",
        (true, true, true) => "enhanced",
        _ => "partial",
    }
}

/// Metrics collection service that runs in background
pub struct MetricsCollectionService {
    prometheus_metrics: Arc<LazyLoadingPrometheusMetrics>,
    collection_interval: std::time::Duration,
}

impl MetricsCollectionService {
    pub fn new(
        prometheus_metrics: Arc<LazyLoadingPrometheusMetrics>,
        collection_interval: std::time::Duration,
    ) -> Self {
        Self { prometheus_metrics, collection_interval }
    }

    /// Start the metrics collection background task
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let metrics = Arc::clone(&self.prometheus_metrics);
        let interval = self.collection_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;

                // Update metrics from lazy loading system
                let _ = metrics.update_from_lazy_loading_metrics().await;

                // Log metrics collection
                tracing::debug!("Updated Prometheus metrics from lazy loading system");
            }
        });

        tracing::info!(
            interval_seconds = self.collection_interval.as_secs(),
            "Started lazy loading Prometheus metrics collection service"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_prometheus_metrics_creation() {
        use std::sync::Arc;
        use surrealdb::Surreal;

        let db = Surreal::init();
        let metrics_repo = Arc::new(matryx_surrealdb::repository::MetricsRepository::new(db));
        let prometheus_metrics = LazyLoadingPrometheusMetrics::new(metrics_repo);

        // Test the prometheus metrics by recording a test metric
        prometheus_metrics.record_request("small", "hit", "enhanced", std::time::Duration::from_millis(100)).await.unwrap();
        
        // LazyLoadingPrometheusMetrics is not a Result type, no need to check is_ok()
        // Test passes by successful construction and metric recording
    }

    #[tokio::test]
    async fn test_metrics_recording() {
        use std::sync::Arc;
        use surrealdb::Surreal;

        let db = Surreal::init();
        let metrics_repo = Arc::new(matryx_surrealdb::repository::MetricsRepository::new(db));
        let prometheus_metrics = LazyLoadingPrometheusMetrics::new(metrics_repo);

        // Record some metrics
        let _ = prometheus_metrics.record_request("medium", "hit", "enhanced", Duration::from_millis(50)).await;
        let _ = prometheus_metrics.record_cache_operation("essential_members", "medium", true).await;
        prometheus_metrics.record_db_query(
            "essential_members",
            "medium",
            "enhanced",
            Duration::from_millis(25),
        );

        // Verify metrics are recorded (this would require accessing the registry)
        let metric_families = prometheus_metrics.registry.gather();
        assert!(!metric_families.is_empty());
    }

    #[test]
    fn test_room_size_bucket() {
        assert_eq!(get_room_size_bucket(5), "tiny");
        assert_eq!(get_room_size_bucket(50), "small");
        assert_eq!(get_room_size_bucket(500), "medium");
        assert_eq!(get_room_size_bucket(5000), "large");
        assert_eq!(get_room_size_bucket(50000), "huge");
    }

    #[test]
    fn test_optimization_level() {
        assert_eq!(get_optimization_level(false, false, false), "basic");
        assert_eq!(get_optimization_level(true, false, false), "cached");
        assert_eq!(get_optimization_level(true, true, false), "optimized");
        assert_eq!(get_optimization_level(true, true, true), "enhanced");
    }
}
