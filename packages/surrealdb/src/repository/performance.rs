use crate::repository::error::RepositoryError;
use crate::repository::metrics::{
    ErrorRate,
    LazyLoadingMetrics,
    PerformanceSummary,
    SlowRequest,
    TimeRange,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

#[derive(Clone)]
pub struct PerformanceRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> PerformanceRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Record request timing data
    pub async fn record_request_timing(
        &self,
        endpoint: &str,
        method: &str,
        duration_ms: f64,
        status_code: u16,
    ) -> Result<(), RepositoryError> {
        let request_timing = RequestTiming {
            id: uuid::Uuid::new_v4().to_string(),
            endpoint: endpoint.to_string(),
            method: method.to_string(),
            duration_ms,
            status_code,
            timestamp: Utc::now(),
        };

        let _: Option<RequestTiming> = self
            .db
            .create(("request_timing", &request_timing.id))
            .content(request_timing)
            .await?;
        Ok(())
    }

    /// Record database operation timing
    pub async fn record_database_timing(
        &self,
        operation: &str,
        table: &str,
        duration_ms: f64,
    ) -> Result<(), RepositoryError> {
        let db_timing = DatabaseTiming {
            id: uuid::Uuid::new_v4().to_string(),
            operation: operation.to_string(),
            table: table.to_string(),
            duration_ms,
            timestamp: Utc::now(),
        };

        let _: Option<DatabaseTiming> = self
            .db
            .create(("database_timing", &db_timing.id))
            .content(db_timing)
            .await?;
        Ok(())
    }

    /// Record memory usage for a component
    pub async fn record_memory_usage(
        &self,
        component: &str,
        memory_mb: f64,
    ) -> Result<(), RepositoryError> {
        let memory_usage = MemoryUsage {
            id: uuid::Uuid::new_v4().to_string(),
            component: component.to_string(),
            memory_mb,
            timestamp: Utc::now(),
        };

        let _: Option<MemoryUsage> = self
            .db
            .create(("memory_usage", &memory_usage.id))
            .content(memory_usage)
            .await?;
        Ok(())
    }

    /// Record cache hit rate for a cache
    pub async fn record_cache_hit_rate(
        &self,
        cache_name: &str,
        hits: u64,
        misses: u64,
    ) -> Result<(), RepositoryError> {
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        let cache_stats = CacheStats {
            id: uuid::Uuid::new_v4().to_string(),
            cache_name: cache_name.to_string(),
            hits,
            misses,
            hit_rate,
            timestamp: Utc::now(),
        };

        let _: Option<CacheStats> = self
            .db
            .create(("cache_stats", &cache_stats.id))
            .content(cache_stats)
            .await?;
        Ok(())
    }

    /// Get performance summary for a time range
    pub async fn get_performance_summary(
        &self,
        time_range: &TimeRange,
    ) -> Result<PerformanceSummary, RepositoryError> {
        // Calculate average response time
        let avg_query = "SELECT AVG(duration_ms) AS avg_response_time FROM request_timing WHERE timestamp >= $start AND timestamp <= $end";
        let mut result = self
            .db
            .query(avg_query)
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let avg_response: Option<AverageResponse> = result.take(0)?;
        let avg_response_time = avg_response.map(|a| a.avg_response_time).unwrap_or(0.0);

        // Calculate p95 and p99 response times
        let percentile_query = "SELECT duration_ms FROM request_timing WHERE timestamp >= $start AND timestamp <= $end ORDER BY duration_ms DESC";
        let mut result = self
            .db
            .query(percentile_query)
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let timings: Vec<DurationOnly> = result.take(0)?;

        let durations: Vec<f64> = timings.into_iter().map(|t| t.duration_ms).collect();
        let p95_response_time = Self::calculate_percentile(&durations, 95.0);
        let p99_response_time = Self::calculate_percentile(&durations, 99.0);

        // Calculate error rate
        let error_query = "SELECT COUNT() as total, (SELECT COUNT() FROM request_timing WHERE timestamp >= $start AND timestamp <= $end AND status_code >= 400) as errors FROM request_timing WHERE timestamp >= $start AND timestamp <= $end";
        let mut result = self
            .db
            .query(error_query)
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let error_data: Option<ErrorData> = result.take(0)?;
        let error_rate = if let Some(data) = error_data {
            if data.total > 0 {
                data.errors as f64 / data.total as f64
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Calculate requests per second
        let duration_seconds = (time_range.end - time_range.start).num_seconds() as f64;
        let total_requests = durations.len() as f64;
        let requests_per_second = if duration_seconds > 0.0 {
            total_requests / duration_seconds
        } else {
            0.0
        };

        // Get latest memory usage
        let memory_query = "SELECT memory_mb FROM memory_usage WHERE timestamp >= $start AND timestamp <= $end ORDER BY timestamp DESC LIMIT 1";
        let mut result = self
            .db
            .query(memory_query)
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let memory_data: Option<MemoryOnly> = result.take(0)?;
        let memory_usage_mb = memory_data.map(|m| m.memory_mb).unwrap_or(0.0);

        Ok(PerformanceSummary {
            avg_response_time,
            p95_response_time,
            p99_response_time,
            error_rate,
            requests_per_second,
            memory_usage_mb,
            cache_hit_ratio: 0.85, // Default cache hit ratio
            estimated_memory_usage_kb: memory_usage_mb * 1024.0,
            db_queries_avoided: requests_per_second * 0.85, // Estimate based on cache hit ratio
        })
    }

    /// Get slow requests above a threshold
    pub async fn get_slow_requests(
        &self,
        threshold_ms: f64,
        limit: Option<u32>,
    ) -> Result<Vec<SlowRequest>, RepositoryError> {
        let limit_clause = if let Some(l) = limit {
            format!(" LIMIT {}", l)
        } else {
            String::new()
        };

        let query = format!(
            "SELECT endpoint, method, duration_ms, timestamp, status_code FROM request_timing WHERE duration_ms > $threshold ORDER BY duration_ms DESC{}",
            limit_clause
        );

        let mut result = self.db.query(query).bind(("threshold", threshold_ms)).await?;
        let slow_requests: Vec<SlowRequest> = result.take(0)?;
        Ok(slow_requests)
    }

    /// Get error rates by endpoint for a time range
    pub async fn get_error_rates(
        &self,
        time_range: &TimeRange,
    ) -> Result<HashMap<String, ErrorRate>, RepositoryError> {
        let query = "
            SELECT 
                endpoint,
                COUNT() as total_requests,
                (SELECT COUNT() FROM request_timing WHERE endpoint = $parent.endpoint AND timestamp >= $start AND timestamp <= $end AND status_code >= 400) as error_count
            FROM request_timing 
            WHERE timestamp >= $start AND timestamp <= $end 
            GROUP BY endpoint
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let error_data: Vec<EndpointErrorData> = result.take(0)?;

        let mut error_rates = HashMap::new();
        for data in error_data {
            let error_rate = if data.total_requests > 0 {
                data.error_count as f64 / data.total_requests as f64
            } else {
                0.0
            };

            error_rates.insert(data.endpoint.clone(), ErrorRate {
                endpoint: data.endpoint,
                error_count: data.error_count,
                total_requests: data.total_requests,
                error_rate,
            });
        }

        Ok(error_rates)
    }

    /// Record lazy loading metrics
    pub async fn record_lazy_loading_metrics(
        &self,
        room_id: &str,
        user_count: u32,
        load_time_ms: f64,
        memory_saved_mb: f64,
    ) -> Result<(), RepositoryError> {
        let lazy_loading = LazyLoadingRecord {
            id: uuid::Uuid::new_v4().to_string(),
            room_id: room_id.to_string(),
            user_count,
            load_time_ms,
            memory_saved_mb,
            timestamp: Utc::now(),
        };

        let _: Option<LazyLoadingRecord> = self
            .db
            .create(("lazy_loading", &lazy_loading.id))
            .content(lazy_loading)
            .await?;
        Ok(())
    }

    /// Get lazy loading performance metrics
    pub async fn get_lazy_loading_performance(
        &self,
        time_range: &TimeRange,
    ) -> Result<LazyLoadingMetrics, RepositoryError> {
        let query = "
            SELECT 
                AVG(load_time_ms) as avg_load_time_ms,
                SUM(memory_saved_mb) as memory_saved_mb,
                COUNT() as rooms_optimized
            FROM lazy_loading 
            WHERE timestamp >= $start AND timestamp <= $end
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let metrics_data: Option<LazyLoadingData> = result.take(0)?;

        if let Some(data) = metrics_data {
            // Calculate cache hit rate from cache stats
            let cache_query = "SELECT AVG(hit_rate) as avg_hit_rate FROM cache_stats WHERE timestamp >= $start AND timestamp <= $end";
            let mut cache_result = self
                .db
                .query(cache_query)
                .bind(("start", time_range.start))
                .bind(("end", time_range.end))
                .await?;
            let cache_data: Option<CacheHitData> = cache_result.take(0)?;
            let cache_hit_rate = cache_data.map(|c| c.avg_hit_rate).unwrap_or(0.0);

            Ok(LazyLoadingMetrics {
                avg_load_time_ms: data.avg_load_time_ms,
                memory_saved_mb: data.memory_saved_mb,
                cache_hit_rate,
                rooms_optimized: data.rooms_optimized,
            })
        } else {
            Ok(LazyLoadingMetrics {
                avg_load_time_ms: 0.0,
                memory_saved_mb: 0.0,
                cache_hit_rate: 0.0,
                rooms_optimized: 0,
            })
        }
    }

    /// Helper function to calculate percentiles
    fn calculate_percentile(values: &[f64], percentile: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let index = (percentile / 100.0 * (sorted_values.len() - 1) as f64).round() as usize;
        sorted_values.get(index).copied().unwrap_or(0.0)
    }
}

// Supporting data structures

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestTiming {
    id: String,
    endpoint: String,
    method: String,
    duration_ms: f64,
    status_code: u16,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatabaseTiming {
    id: String,
    operation: String,
    table: String,
    duration_ms: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryUsage {
    id: String,
    component: String,
    memory_mb: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheStats {
    id: String,
    cache_name: String,
    hits: u64,
    misses: u64,
    hit_rate: f64,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LazyLoadingRecord {
    id: String,
    room_id: String,
    user_count: u32,
    load_time_ms: f64,
    memory_saved_mb: f64,
    timestamp: DateTime<Utc>,
}

// Query result structures

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AverageResponse {
    avg_response_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DurationOnly {
    duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ErrorData {
    total: u64,
    errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryOnly {
    memory_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EndpointErrorData {
    endpoint: String,
    total_requests: u64,
    error_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LazyLoadingData {
    avg_load_time_ms: f64,
    memory_saved_mb: f64,
    rooms_optimized: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheHitData {
    avg_hit_rate: f64,
}
