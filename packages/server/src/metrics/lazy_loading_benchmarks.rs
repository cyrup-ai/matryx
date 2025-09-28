use rand;
use std::collections::HashMap;
use std::time::Duration;
use tracing::warn;

/// Performance benchmarking configuration
pub struct LazyLoadingBenchmarkConfig {
    /// Target performance thresholds
    pub max_db_query_time_ms: u64,
    pub max_total_processing_time_ms: u64,
    pub min_cache_hit_ratio: f64,
    pub max_cache_memory_mb: usize,

    /// Room size buckets for performance analysis
    pub room_size_buckets: Vec<(String, usize)>, // [(bucket_name, max_members)]

    /// Sampling configuration
    pub sample_rate: f64, // 0.0 to 1.0 - percentage of operations to track
    pub max_samples_per_bucket: usize,
}

impl Default for LazyLoadingBenchmarkConfig {
    fn default() -> Self {
        Self {
            max_db_query_time_ms: 100,
            max_total_processing_time_ms: 500,
            min_cache_hit_ratio: 0.8,
            max_cache_memory_mb: 256,
            room_size_buckets: vec![
                ("small".to_string(), 100),
                ("medium".to_string(), 1000),
                ("large".to_string(), 10000),
                ("xlarge".to_string(), usize::MAX),
            ],
            sample_rate: 0.1, // Sample 10% of operations
            max_samples_per_bucket: 1000,
        }
    }
}

/// Runtime benchmark result
#[derive(Debug, Clone)]
pub struct LazyLoadingBenchmarkResult {
    pub room_size_bucket: String,
    pub operation_type: String,
    pub cache_status: String, // "hit", "miss", "invalidation"
    pub db_query_duration_ms: u64,
    pub total_duration_ms: u64,
    pub timestamp: u64,
}

/// Thread-safe benchmarking data aggregator
pub struct LazyLoadingBenchmarks {
    config: LazyLoadingBenchmarkConfig,
    results: std::sync::Mutex<Vec<LazyLoadingBenchmarkResult>>,
}

impl LazyLoadingBenchmarks {
    pub fn new(config: LazyLoadingBenchmarkConfig) -> Self {
        Self { config, results: std::sync::Mutex::new(Vec::new()) }
    }

    pub fn with_default_config() -> Self {
        Self::new(LazyLoadingBenchmarkConfig::default())
    }

    /// Record a lazy loading operation benchmark
    pub fn record_operation(
        &self,
        room_members_count: usize,
        operation_type: &str,
        cache_status: &str,
        db_query_duration: Duration,
        total_duration: Duration,
    ) {
        // Sample based on configured rate
        if rand::random::<f64>() > self.config.sample_rate {
            return;
        }

        let room_size_bucket = self.get_room_size_bucket(room_members_count);

        let result = LazyLoadingBenchmarkResult {
            room_size_bucket,
            operation_type: operation_type.to_string(),
            cache_status: cache_status.to_string(),
            db_query_duration_ms: db_query_duration.as_millis() as u64,
            total_duration_ms: total_duration.as_millis() as u64,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let mut results = self.results.lock().unwrap_or_else(|poisoned| {
            warn!("Benchmark results mutex was poisoned, recovering with data");
            poisoned.into_inner()
        });

        // Maintain sample limit per bucket
        let bucket_samples = results
            .iter()
            .filter(|r| r.room_size_bucket == result.room_size_bucket)
            .count();

        if bucket_samples < self.config.max_samples_per_bucket {
            results.push(result);
        }
    }

    fn get_room_size_bucket(&self, member_count: usize) -> String {
        for (bucket_name, max_size) in &self.config.room_size_buckets {
            if member_count <= *max_size {
                return bucket_name.clone();
            }
        }
        "unknown".to_string()
    }
}

/// Performance histogram aggregator for detailed analysis
#[derive(Debug, Default)]
pub struct LazyLoadingPerformanceHistogram {
    lazy_loading_duration_histogram: HashMap<String, Vec<f64>>,
    db_query_duration_histogram: HashMap<String, Vec<f64>>,
}

impl LazyLoadingPerformanceHistogram {
    pub fn new() -> Self {
        Self {
            lazy_loading_duration_histogram: HashMap::new(),
            db_query_duration_histogram: HashMap::new(),
        }
    }

    pub fn record_lazy_loading_operation(
        &mut self,
        room_size_bucket: &str,
        cache_status: &str,
        duration: Duration,
    ) {
        let key = format!("{}_{}", room_size_bucket, cache_status);
        self.lazy_loading_duration_histogram
            .entry(key)
            .or_default()
            .push(duration.as_secs_f64() * 1000.0); // Convert to milliseconds
    }

    pub fn record_db_query(
        &mut self,
        query_type: &str,
        room_size_bucket: &str,
        duration: Duration,
    ) {
        let key = format!("{}_{}", room_size_bucket, query_type);
        self.db_query_duration_histogram
            .entry(key)
            .or_default()
            .push(duration.as_secs_f64() * 1000.0); // Convert to milliseconds
    }
}

/// Helper struct for benchmark analysis
#[derive(Debug)]
pub struct LazyLoadingBenchmarkAnalysis {
    pub avg_db_query_time_ms: f64,
    pub p95_db_query_time_ms: f64,
    pub avg_total_time_ms: f64,
    pub p95_total_time_ms: f64,
    pub cache_hit_ratio: f64,
    pub total_operations: usize,
    pub performance_issues: Vec<String>,
}

impl LazyLoadingBenchmarks {
    /// Generate comprehensive performance analysis
    pub fn analyze_performance(&self) -> HashMap<String, LazyLoadingBenchmarkAnalysis> {
        let results = self.results.lock().unwrap_or_else(|poisoned| {
            warn!("Benchmark results mutex was poisoned during analysis, recovering with data");
            poisoned.into_inner()
        });
        let mut analysis_by_bucket = HashMap::new();

        // Group results by room size bucket
        let mut bucket_results: HashMap<String, Vec<&LazyLoadingBenchmarkResult>> = HashMap::new();
        for result in results.iter() {
            bucket_results
                .entry(result.room_size_bucket.clone())
                .or_default()
                .push(result);
        }

        for (bucket, bucket_data) in bucket_results {
            let analysis = self.analyze_bucket_performance(&bucket_data);
            analysis_by_bucket.insert(bucket, analysis);
        }

        analysis_by_bucket
    }

    fn analyze_bucket_performance(
        &self,
        results: &[&LazyLoadingBenchmarkResult],
    ) -> LazyLoadingBenchmarkAnalysis {
        if results.is_empty() {
            return LazyLoadingBenchmarkAnalysis {
                avg_db_query_time_ms: 0.0,
                p95_db_query_time_ms: 0.0,
                avg_total_time_ms: 0.0,
                p95_total_time_ms: 0.0,
                cache_hit_ratio: 0.0,
                total_operations: 0,
                performance_issues: vec!["No data available".to_string()],
            };
        }

        // Calculate statistics
        let total_operations = results.len();
        let db_times: Vec<u64> = results.iter().map(|r| r.db_query_duration_ms).collect();
        let total_times: Vec<u64> = results.iter().map(|r| r.total_duration_ms).collect();

        let avg_db_query_time_ms = db_times.iter().sum::<u64>() as f64 / total_operations as f64;
        let avg_total_time_ms = total_times.iter().sum::<u64>() as f64 / total_operations as f64;

        let p95_db_query_time_ms = self.calculate_percentile(&db_times, 0.95);
        let p95_total_time_ms = self.calculate_percentile(&total_times, 0.95);

        let cache_hits = results.iter().filter(|r| r.cache_status == "hit").count();
        let cache_hit_ratio = cache_hits as f64 / total_operations as f64;

        // Identify performance issues
        let mut performance_issues = Vec::new();
        if avg_db_query_time_ms > self.config.max_db_query_time_ms as f64 {
            performance_issues.push(format!(
                "Average DB query time ({:.2}ms) exceeds threshold ({}ms)",
                avg_db_query_time_ms, self.config.max_db_query_time_ms
            ));
        }

        if avg_total_time_ms > self.config.max_total_processing_time_ms as f64 {
            performance_issues.push(format!(
                "Average total processing time ({:.2}ms) exceeds threshold ({}ms)",
                avg_total_time_ms, self.config.max_total_processing_time_ms
            ));
        }

        if cache_hit_ratio < self.config.min_cache_hit_ratio {
            performance_issues.push(format!(
                "Cache hit ratio ({:.2}) below threshold ({:.2})",
                cache_hit_ratio, self.config.min_cache_hit_ratio
            ));
        }

        LazyLoadingBenchmarkAnalysis {
            avg_db_query_time_ms,
            p95_db_query_time_ms,
            avg_total_time_ms,
            p95_total_time_ms,
            cache_hit_ratio,
            total_operations,
            performance_issues,
        }
    }

    fn calculate_percentile(&self, values: &[u64], percentile: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let mut sorted_values = values.to_vec();
        sorted_values.sort_unstable();

        let index = (percentile * (sorted_values.len() - 1) as f64).round() as usize;
        sorted_values[index.min(sorted_values.len() - 1)] as f64
    }

    /// Export benchmark data for external analysis
    pub fn export_csv(&self) -> String {
        let results = self.results.lock().unwrap_or_else(|poisoned| {
            warn!("Benchmark results mutex was poisoned during CSV export, recovering with data");
            poisoned.into_inner()
        });
        let mut csv = "timestamp,room_size_bucket,operation_type,cache_status,db_query_duration_ms,total_duration_ms\n".to_string();

        for result in results.iter() {
            csv.push_str(&format!(
                "{},{},{},{},{},{}\n",
                result.timestamp,
                result.room_size_bucket,
                result.operation_type,
                result.cache_status,
                result.db_query_duration_ms,
                result.total_duration_ms
            ));
        }

        csv
    }

    /// Reset all collected benchmark data
    pub fn reset(&self) {
        let mut results = self.results.lock().unwrap_or_else(|poisoned| {
            warn!("Benchmark results mutex was poisoned during reset, recovering with data");
            poisoned.into_inner()
        });
        results.clear();
    }

    /// Get current data collection statistics
    pub fn get_stats(&self) -> (usize, HashMap<String, usize>) {
        let results = self.results.lock().unwrap_or_else(|poisoned| {
            warn!("Benchmark results mutex was poisoned during stats collection, recovering with data");
            poisoned.into_inner()
        });
        let total = results.len();

        let mut by_bucket = HashMap::new();
        for result in results.iter() {
            *by_bucket.entry(result.room_size_bucket.clone()).or_insert(0) += 1;
        }

        (total, by_bucket)
    }
}

/// Global benchmark instance for application-wide tracking
static GLOBAL_BENCHMARKS: std::sync::OnceLock<LazyLoadingBenchmarks> = std::sync::OnceLock::new();

/// Global benchmark access functions
pub fn init_benchmarks(config: LazyLoadingBenchmarkConfig) {
    GLOBAL_BENCHMARKS
        .set(LazyLoadingBenchmarks::new(config))
        .unwrap_or(());
}

pub fn record_lazy_loading_operation(
    room_members_count: usize,
    operation_type: &str,
    cache_status: &str,
    db_query_duration: Duration,
    total_duration: Duration,
) {
    if let Some(benchmarks) = GLOBAL_BENCHMARKS.get() {
        benchmarks.record_operation(
            room_members_count,
            operation_type,
            cache_status,
            db_query_duration,
            total_duration,
        );
    }
}

pub fn record_db_query(query_type: &str, room_size_bucket: &str, duration: Duration) {
    // This function exists for compatibility but could be enhanced
    // to track database queries separately if needed
    if let Some(_benchmarks) = GLOBAL_BENCHMARKS.get() {
        // Could implement separate DB query tracking here
        tracing::debug!(
            query_type = query_type,
            room_size_bucket = room_size_bucket,
            duration_ms = duration.as_millis(),
            "Database query recorded"
        );
    }
}

pub fn get_benchmark_analysis() -> Option<HashMap<String, LazyLoadingBenchmarkAnalysis>> {
    GLOBAL_BENCHMARKS.get().map(|b| b.analyze_performance())
}

pub fn export_benchmark_csv() -> Option<String> {
    GLOBAL_BENCHMARKS.get().map(|b| b.export_csv())
}

pub fn reset_benchmarks() {
    if let Some(benchmarks) = GLOBAL_BENCHMARKS.get() {
        benchmarks.reset();
    }
}

pub fn get_benchmark_stats() -> Option<(usize, HashMap<String, usize>)> {
    GLOBAL_BENCHMARKS.get().map(|b| b.get_stats())
}
