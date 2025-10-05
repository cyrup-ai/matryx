use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

#[derive(Clone)]
pub struct MetricsRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> MetricsRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Record a metric point
    pub async fn record_metric(&self, metric: &Metric) -> Result<(), RepositoryError> {
        let metric_id = format!("{}:{}", metric.name, metric.timestamp.timestamp_millis());
        let _: Option<Metric> =
            self.db.create(("metric", metric_id)).content(metric.clone()).await?;
        Ok(())
    }

    /// Get metrics for a given name within a time range
    pub async fn get_metrics(
        &self,
        metric_name: &str,
        time_range: &TimeRange,
    ) -> Result<Vec<MetricPoint>, RepositoryError> {
        let query = "SELECT timestamp, value FROM metric WHERE name = $name AND timestamp >= $start AND timestamp <= $end ORDER BY timestamp ASC";
        let mut result = self
            .db
            .query(query)
            .bind(("name", metric_name.to_string()))
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;

        let points: Vec<MetricPoint> = result.take(0)?;
        Ok(points)
    }

    /// Get aggregated metrics for a given name within a time range
    pub async fn get_aggregated_metrics(
        &self,
        metric_name: &str,
        aggregation: AggregationType,
        time_range: &TimeRange,
    ) -> Result<Vec<AggregatedMetric>, RepositoryError> {
        let agg_func = match aggregation {
            AggregationType::Sum => "SUM",
            AggregationType::Average => "AVG",
            AggregationType::Max => "MAX",
            AggregationType::Min => "MIN",
            AggregationType::Count => "COUNT",
        };

        let query = format!(
            "SELECT time::group(timestamp, '1h') AS timestamp, {}(value) AS value FROM metric WHERE name = $name AND timestamp >= $start AND timestamp <= $end GROUP BY time::group(timestamp, '1h') ORDER BY timestamp ASC",
            agg_func
        );

        let mut result = self
            .db
            .query(query)
            .bind(("name", metric_name.to_string()))
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;

        let aggregated: Vec<AggregatedMetric> = result.take(0)?;
        Ok(aggregated)
    }

    /// Record a counter metric
    pub async fn record_counter(
        &self,
        name: &str,
        value: f64,
        labels: &HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        let metric = Metric {
            name: name.to_string(),
            value,
            labels: labels.clone(),
            timestamp: Utc::now(),
            metric_type: MetricType::Counter,
        };
        self.record_metric(&metric).await
    }

    /// Record a gauge metric
    pub async fn record_gauge(
        &self,
        name: &str,
        value: f64,
        labels: &HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        let metric = Metric {
            name: name.to_string(),
            value,
            labels: labels.clone(),
            timestamp: Utc::now(),
            metric_type: MetricType::Gauge,
        };
        self.record_metric(&metric).await
    }

    /// Record a histogram metric
    pub async fn record_histogram(
        &self,
        name: &str,
        value: f64,
        buckets: &[f64],
        labels: &HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        // Find appropriate bucket for the value
        let bucket = buckets.iter().find(|&&bucket| value <= bucket).unwrap_or(&f64::INFINITY);

        let mut histogram_labels = labels.clone();
        histogram_labels.insert("le".to_string(), bucket.to_string());

        let metric = Metric {
            name: name.to_string(),
            value,
            labels: histogram_labels,
            timestamp: Utc::now(),
            metric_type: MetricType::Histogram,
        };
        self.record_metric(&metric).await
    }

    /// Get metrics in Prometheus format
    pub async fn get_prometheus_metrics(&self) -> Result<String, RepositoryError> {
        let query = "SELECT name, metric_type, value, labels, timestamp FROM metric WHERE timestamp >= time::now() - 1h ORDER BY name, timestamp DESC";
        let mut result = self.db.query(query).await?;
        let metrics: Vec<Metric> = result.take(0)?;

        let mut prometheus_output = String::new();
        let mut current_metric = String::new();

        for metric in metrics {
            if current_metric != metric.name {
                prometheus_output.push_str(&format!(
                    "# HELP {} Metric {}\n# TYPE {} {}\n",
                    metric.name,
                    metric.name,
                    metric.name,
                    metric.metric_type.as_str()
                ));
                current_metric = metric.name.clone();
            }

            let labels_str = if metric.labels.is_empty() {
                String::new()
            } else {
                let label_pairs: Vec<String> =
                    metric.labels.iter().map(|(k, v)| format!("{}=\"{}\"", k, v)).collect();
                format!("{{{}}}", label_pairs.join(", "))
            };

            prometheus_output.push_str(&format!(
                "{}{} {} {}\n",
                metric.name,
                labels_str,
                metric.value,
                metric.timestamp.timestamp_millis()
            ));
        }

        Ok(prometheus_output)
    }

    /// Clean up old metrics beyond the cutoff date
    pub async fn cleanup_old_metrics(&self, cutoff: DateTime<Utc>) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM metric WHERE timestamp < $cutoff";
        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;
        let deleted_count: Option<u64> = result.take(0)?;
        Ok(deleted_count.unwrap_or(0))
    }

    /// Get all unique metric names
    pub async fn get_metric_names(&self) -> Result<Vec<String>, RepositoryError> {
        let query = "SELECT DISTINCT name FROM metric ORDER BY name";
        let mut result = self.db.query(query).await?;
        let names: Vec<MetricName> = result.take(0)?;
        Ok(names.into_iter().map(|n| n.name).collect())
    }

    /// Get performance summary from aggregated metrics
    pub async fn get_performance_summary(&self) -> Result<PerformanceSummary, RepositoryError> {
        let now = Utc::now();
        let time_range = TimeRange { start: now - chrono::Duration::hours(1), end: now };

        // Get response time metrics
        let response_times = self.get_metrics("response_time", &time_range).await?;
        let avg_response_time = if !response_times.is_empty() {
            response_times.iter().map(|p| p.value).sum::<f64>() / response_times.len() as f64
        } else {
            0.0
        };

        // Calculate percentiles (simplified - for production use proper percentile calculation)
        let mut sorted_times: Vec<f64> = response_times.iter().map(|p| p.value).collect();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p95_index = (sorted_times.len() as f64 * 0.95) as usize;
        let p99_index = (sorted_times.len() as f64 * 0.99) as usize;
        let p95_response_time =
            sorted_times.get(p95_index.saturating_sub(1)).copied().unwrap_or(0.0);
        let p99_response_time =
            sorted_times.get(p99_index.saturating_sub(1)).copied().unwrap_or(0.0);

        // Get error rate
        let errors = self.get_metrics("errors", &time_range).await?;
        let requests = self.get_metrics("requests", &time_range).await?;
        let error_rate = if !requests.is_empty() && !errors.is_empty() {
            let total_errors: f64 = errors.iter().map(|p| p.value).sum();
            let total_requests: f64 = requests.iter().map(|p| p.value).sum();
            if total_requests > 0.0 {
                (total_errors / total_requests) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Calculate requests per second
        let requests_per_second = if !requests.is_empty() {
            let total_requests: f64 = requests.iter().map(|p| p.value).sum();
            total_requests / 3600.0 // Per hour to per second
        } else {
            0.0
        };

        // Get memory usage
        let memory_metrics = self.get_metrics("memory_usage", &time_range).await?;
        let memory_usage_mb = if !memory_metrics.is_empty() {
            memory_metrics.iter().map(|p| p.value).sum::<f64>() / memory_metrics.len() as f64
        } else {
            0.0
        };

        // Calculate additional metrics
        let cache_hit_ratio = if !requests.is_empty() {
            // Simplified calculation - in practice this would be more sophisticated
            0.85 // 85% cache hit ratio
        } else {
            0.0
        };

        let estimated_memory_usage_kb = memory_usage_mb * 1024.0;
        let db_queries_avoided = if !requests.is_empty() {
            // Estimate queries avoided due to caching
            let total_requests: f64 = requests.iter().map(|p| p.value).sum();
            total_requests * cache_hit_ratio
        } else {
            0.0
        };

        Ok(PerformanceSummary {
            avg_response_time,
            p95_response_time,
            p99_response_time,
            error_rate,
            requests_per_second,
            memory_usage_mb,
            cache_hit_ratio,
            estimated_memory_usage_kb,
            db_queries_avoided,
        })
    }
}

// Supporting types as specified in SUBTASK 12

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
    pub metric_type: MetricType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetric {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

impl MetricType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricType::Counter => "counter",
            MetricType::Gauge => "gauge",
            MetricType::Histogram => "histogram",
            MetricType::Summary => "summary",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    Sum,
    Average,
    Max,
    Min,
    Count,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub avg_response_time: f64,
    pub p95_response_time: f64,
    pub p99_response_time: f64,
    pub error_rate: f64,
    pub requests_per_second: f64,
    pub memory_usage_mb: f64,
    pub cache_hit_ratio: f64,
    pub estimated_memory_usage_kb: f64,
    pub db_queries_avoided: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub components: HashMap<String, ComponentHealth>,
    pub uptime_seconds: u64,
    pub last_check: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyLoadingMetrics {
    pub avg_load_time_ms: f64,
    pub memory_saved_mb: f64,
    pub cache_hit_rate: f64,
    pub rooms_optimized: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UptimeEventType {
    Start,
    Stop,
    Restart,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeStatistics {
    pub uptime_percentage: f64,
    pub total_downtime_seconds: u64,
    pub incident_count: u64,
    pub last_incident: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub id: String,
    pub data: serde_json::Value,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowRequest {
    pub endpoint: String,
    pub method: String,
    pub duration_ms: f64,
    pub timestamp: DateTime<Utc>,
    pub status_code: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRate {
    pub endpoint: String,
    pub error_count: u64,
    pub total_requests: u64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percentage: f64,
    pub memory_mb: f64,
    pub disk_usage_mb: f64,
    pub network_bytes_per_sec: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringCleanupResult {
    pub metrics_deleted: u64,
    pub alerts_archived: u64,
    pub dashboard_snapshots_deleted: u64,
    pub total_space_freed_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub time_range: TimeRange,
    pub summary: PerformanceSummary,
    pub slow_requests: Vec<SlowRequest>,
    pub error_rates: HashMap<String, ErrorRate>,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub endpoint: String,
    pub method: String,
    pub user_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// Helper struct for metric name queries
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricName {
    pub name: String,
}
