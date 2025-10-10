use crate::repository::error::RepositoryError;
use crate::repository::{
    Alert,
    AlertSeverity,
    ApiRequest,
    DashboardData,
    MetricsRepository,
    MonitoringCleanupResult,
    MonitoringRepository,
    PerformanceReport,
    PerformanceRepository,
    ResourceUsage,
    TimeRange,
};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use surrealdb::{Connection, Surreal};
use sysinfo::{Disks, Networks, System};
use tokio::sync::{Mutex, RwLock};

/// Cached CPU reading with timestamp for time-based expiration
#[derive(Clone)]
struct CachedCpuReading {
    /// CPU usage percentage (0.0 - 100.0)
    value: f64,
    
    /// When this reading was captured
    timestamp: Instant,
}

/// Thread-safe cache for CPU usage metrics
/// 
/// Uses RwLock to allow concurrent reads of cached values while
/// serializing writes when the cache expires and needs updating.
/// 
/// Design rationale:
/// - High read frequency (100s-1000s of monitoring calls per second)
/// - Low write frequency (once every ~5 seconds when cache expires)
/// - RwLock provides better performance than Mutex for this access pattern
#[derive(Clone)]
struct CpuMetricsCache {
    /// Last CPU reading with timestamp
    last_reading: Arc<RwLock<Option<CachedCpuReading>>>,
    
    /// How long cached values remain valid
    cache_duration: std::time::Duration,
}

impl CpuMetricsCache {
    /// Create a new cache with specified duration
    fn new(cache_duration: std::time::Duration) -> Self {
        Self {
            last_reading: Arc::new(RwLock::new(None)),
            cache_duration,
        }
    }
    
    /// Get cached value if still valid
    /// 
    /// Returns Some(value) if cache hit, None if cache miss or expired.
    /// Uses read lock for concurrent access by multiple callers.
    async fn get(&self) -> Option<f64> {
        let cache = self.last_reading.read().await;
        
        if let Some(reading) = cache.as_ref() {
            if reading.timestamp.elapsed() < self.cache_duration {
                tracing::trace!(
                    cpu_usage = reading.value,
                    cache_age_ms = reading.timestamp.elapsed().as_millis(),
                    "CPU cache hit"
                );
                return Some(reading.value);
            } else {
                tracing::trace!(
                    cache_age_ms = reading.timestamp.elapsed().as_millis(),
                    cache_duration_ms = self.cache_duration.as_millis(),
                    "CPU cache expired"
                );
            }
        }
        
        None
    }
    
    /// Store a new reading in the cache
    /// 
    /// Uses write lock to update cache atomically.
    /// Only one writer can update at a time.
    async fn set(&self, value: f64) {
        let mut cache = self.last_reading.write().await;
        *cache = Some(CachedCpuReading {
            value,
            timestamp: Instant::now(),
        });
        
        tracing::trace!(
            cpu_usage = value,
            "CPU cache updated with fresh measurement"
        );
    }
}

/// Cached memory metrics
#[derive(Clone)]
struct CachedMemoryReading {
    value: f64,  // Memory in MB
    timestamp: Instant,
}

/// Memory metrics cache (5 second duration - changes moderately)
#[derive(Clone)]
struct MemoryMetricsCache {
    last_reading: Arc<RwLock<Option<CachedMemoryReading>>>,
    cache_duration: std::time::Duration,
}

impl MemoryMetricsCache {
    fn new(cache_duration: std::time::Duration) -> Self {
        Self {
            last_reading: Arc::new(RwLock::new(None)),
            cache_duration,
        }
    }
    
    async fn get(&self) -> Option<f64> {
        let cache = self.last_reading.read().await;
        if let Some(reading) = cache.as_ref() {
            if reading.timestamp.elapsed() < self.cache_duration {
                tracing::trace!(memory_mb = reading.value, "Memory cache hit");
                return Some(reading.value);
            }
        }
        None
    }
    
    async fn set(&self, value: f64) {
        let mut cache = self.last_reading.write().await;
        *cache = Some(CachedMemoryReading {
            value,
            timestamp: Instant::now(),
        });
        tracing::trace!(memory_mb = value, "Memory cache updated");
    }
}

/// Cached disk metrics  
#[derive(Clone)]
struct CachedDiskReading {
    value: f64,  // Disk usage in MB
    timestamp: Instant,
}

/// Disk metrics cache (30 second duration - changes slowly)
#[derive(Clone)]
struct DiskMetricsCache {
    last_reading: Arc<RwLock<Option<CachedDiskReading>>>,
    cache_duration: std::time::Duration,
}

impl DiskMetricsCache {
    fn new(cache_duration: std::time::Duration) -> Self {
        Self {
            last_reading: Arc::new(RwLock::new(None)),
            cache_duration,
        }
    }
    
    async fn get(&self) -> Option<f64> {
        let cache = self.last_reading.read().await;
        if let Some(reading) = cache.as_ref() {
            if reading.timestamp.elapsed() < self.cache_duration {
                tracing::trace!(disk_mb = reading.value, "Disk cache hit");
                return Some(reading.value);
            }
        }
        None
    }
    
    async fn set(&self, value: f64) {
        let mut cache = self.last_reading.write().await;
        *cache = Some(CachedDiskReading {
            value,
            timestamp: Instant::now(),
        });
        tracing::trace!(disk_mb = value, "Disk cache updated");
    }
}

#[derive(Clone)]
pub struct MonitoringService<C: Connection> {
    metrics_repo: MetricsRepository<C>,
    performance_repo: PerformanceRepository<C>,
    monitoring_repo: MonitoringRepository<C>,
    last_network_stats: Arc<Mutex<Option<(Instant, u64, u64)>>>,
    
    /// Cache for CPU metrics to reduce spawn_blocking overhead
    /// 
    /// Caches CPU readings for 5 seconds to prevent thread pool exhaustion.
    /// See CPUCACHE_1 task for rationale and implementation details.
    cpu_cache: CpuMetricsCache,
    
    /// Cache for memory metrics to reduce spawn_blocking overhead
    memory_cache: MemoryMetricsCache,
    
    /// Cache for disk metrics to reduce spawn_blocking overhead
    disk_cache: DiskMetricsCache,
}

impl<C: Connection> MonitoringService<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            metrics_repo: MetricsRepository::new(db.clone()),
            performance_repo: PerformanceRepository::new(db.clone()),
            monitoring_repo: MonitoringRepository::new(db),
            last_network_stats: Arc::new(Mutex::new(None)),
            cpu_cache: CpuMetricsCache::new(std::time::Duration::from_secs(5)),
            memory_cache: MemoryMetricsCache::new(std::time::Duration::from_secs(5)),
            disk_cache: DiskMetricsCache::new(std::time::Duration::from_secs(30)),
        }
    }
    
    /// Create monitoring service with custom CPU cache duration
    /// 
    /// Useful for different monitoring frequencies or testing scenarios.
    /// Default duration is 5 seconds (see `new()`).
    /// 
    /// # Arguments
    /// * `db` - SurrealDB connection
    /// * `cpu_cache_duration` - How long CPU readings remain valid
    pub fn with_cpu_cache_duration(db: Surreal<C>, cpu_cache_duration: std::time::Duration) -> Self {
        Self {
            metrics_repo: MetricsRepository::new(db.clone()),
            performance_repo: PerformanceRepository::new(db.clone()),
            monitoring_repo: MonitoringRepository::new(db),
            last_network_stats: Arc::new(Mutex::new(None)),
            cpu_cache: CpuMetricsCache::new(cpu_cache_duration),
            memory_cache: MemoryMetricsCache::new(std::time::Duration::from_secs(5)),
            disk_cache: DiskMetricsCache::new(std::time::Duration::from_secs(30)),
        }
    }

    /// Collect comprehensive system metrics
    pub async fn collect_system_metrics(&self) -> Result<(), RepositoryError> {
        let _timestamp = Utc::now();

        // Collect CPU usage
        let cpu_usage = self.get_current_cpu_usage().await?;
        self.metrics_repo
            .record_gauge("system_cpu_usage_percent", cpu_usage, &HashMap::new())
            .await?;

        // Collect memory usage
        let memory_usage = self.get_current_memory_usage().await?;
        self.metrics_repo
            .record_gauge("system_memory_usage_mb", memory_usage, &HashMap::new())
            .await?;

        // Record memory usage in performance repo
        self.performance_repo.record_memory_usage("system", memory_usage).await?;

        // Collect disk usage
        let disk_usage = self.get_current_disk_usage().await?;
        self.metrics_repo
            .record_gauge("system_disk_usage_mb", disk_usage, &HashMap::new())
            .await?;

        // Collect network metrics
        let network_bytes_per_sec = self.get_network_throughput().await?;
        self.metrics_repo
            .record_gauge("system_network_bytes_per_sec", network_bytes_per_sec, &HashMap::new())
            .await?;

        // Record system health check
        let health_status = if cpu_usage < 80.0 && memory_usage < 1024.0 * 8.0 {
            crate::repository::metrics::HealthStatus::Healthy
        } else if cpu_usage < 95.0 && memory_usage < 1024.0 * 12.0 {
            crate::repository::metrics::HealthStatus::Degraded
        } else {
            crate::repository::metrics::HealthStatus::Unhealthy
        };

        self.monitoring_repo
            .record_health_check("system", health_status, None)
            .await?;

        Ok(())
    }

    /// Generate comprehensive performance report
    pub async fn generate_performance_report(
        &self,
        time_range: &TimeRange,
    ) -> Result<PerformanceReport, RepositoryError> {
        // Get performance summary
        let summary = self.performance_repo.get_performance_summary(time_range).await?;

        // Get slow requests
        let slow_requests = self.performance_repo.get_slow_requests(1000.0, Some(50)).await?;

        // Get error rates
        let error_rates = self.performance_repo.get_error_rates(time_range).await?;

        // Get latest resource usage
        let resource_usage = self.get_current_resource_usage().await?;

        Ok(PerformanceReport {
            time_range: time_range.clone(),
            summary,
            slow_requests,
            error_rates,
            resource_usage,
        })
    }

    /// Check alert conditions and generate alerts if needed
    pub async fn check_alert_conditions(&self) -> Result<Vec<Alert>, RepositoryError> {
        let mut generated_alerts = Vec::new();
        let now = Utc::now();
        let one_hour_ago = now - Duration::hours(1);
        let time_range = TimeRange { start: one_hour_ago, end: now };

        // Check performance thresholds
        let summary = self.performance_repo.get_performance_summary(&time_range).await?;

        // Alert on high response times
        if summary.avg_response_time > 2000.0 {
            let alert = Alert {
                id: String::new(), // Will be set by monitoring_repo
                name: "High Average Response Time".to_string(),
                severity: AlertSeverity::Warning,
                message: format!(
                    "Average response time is {:.2}ms, exceeding 2000ms threshold",
                    summary.avg_response_time
                ),
                created_at: now,
                resolved_at: None,
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("metric".to_string(), "response_time".to_string());
                    labels.insert("threshold".to_string(), "2000".to_string());
                    labels
                },
            };

            let alert_id = self.monitoring_repo.create_alert(&alert).await?;
            let mut created_alert = alert;
            created_alert.id = alert_id;
            generated_alerts.push(created_alert);
        }

        // Alert on high error rates
        if summary.error_rate > 0.05 {
            let alert = Alert {
                id: String::new(),
                name: "High Error Rate".to_string(),
                severity: AlertSeverity::Critical,
                message: format!(
                    "Error rate is {:.2}%, exceeding 5% threshold",
                    summary.error_rate * 100.0
                ),
                created_at: now,
                resolved_at: None,
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("metric".to_string(), "error_rate".to_string());
                    labels.insert("threshold".to_string(), "0.05".to_string());
                    labels
                },
            };

            let alert_id = self.monitoring_repo.create_alert(&alert).await?;
            let mut created_alert = alert;
            created_alert.id = alert_id;
            generated_alerts.push(created_alert);
        }

        // Alert on high memory usage
        if summary.memory_usage_mb > 8192.0 {
            let alert = Alert {
                id: String::new(),
                name: "High Memory Usage".to_string(),
                severity: AlertSeverity::Warning,
                message: format!(
                    "Memory usage is {:.2}MB, exceeding 8GB threshold",
                    summary.memory_usage_mb
                ),
                created_at: now,
                resolved_at: None,
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("metric".to_string(), "memory_usage".to_string());
                    labels.insert("threshold".to_string(), "8192".to_string());
                    labels
                },
            };

            let alert_id = self.monitoring_repo.create_alert(&alert).await?;
            let mut created_alert = alert;
            created_alert.id = alert_id;
            generated_alerts.push(created_alert);
        }

        Ok(generated_alerts)
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus_metrics(&self) -> Result<String, RepositoryError> {
        self.metrics_repo.get_prometheus_metrics().await
    }

    /// Generate dashboard data for a specific dashboard
    pub async fn generate_dashboard_data(
        &self,
        dashboard_id: &str,
    ) -> Result<DashboardData, RepositoryError> {
        let now = Utc::now();
        let one_hour_ago = now - Duration::hours(1);
        let time_range = TimeRange { start: one_hour_ago, end: now };

        match dashboard_id {
            "overview" => self.generate_overview_dashboard(&time_range).await,
            "performance" => self.generate_performance_dashboard(&time_range).await,
            "alerts" => self.generate_alerts_dashboard(&time_range).await,
            _ => {
                // Try to get existing dashboard data
                self.monitoring_repo.get_dashboard_data(dashboard_id, &time_range).await
            },
        }
    }

    /// Record an API request with response time and status
    pub async fn record_api_request(
        &self,
        request: &ApiRequest,
        response_time: f64,
        status: u16,
    ) -> Result<(), RepositoryError> {
        // Record in performance repository
        self.performance_repo
            .record_request_timing(&request.endpoint, &request.method, response_time, status)
            .await?;

        // Record as counter in metrics repository
        let mut labels = HashMap::new();
        labels.insert("endpoint".to_string(), request.endpoint.clone());
        labels.insert("method".to_string(), request.method.clone());
        labels.insert("status".to_string(), status.to_string());

        self.metrics_repo
            .record_counter("api_requests_total", 1.0, &labels)
            .await?;

        // Record response time as histogram
        let buckets = vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
        ];
        self.metrics_repo
            .record_histogram(
                "api_request_duration_seconds",
                response_time / 1000.0,
                &buckets,
                &labels,
            )
            .await?;

        Ok(())
    }

    /// Track current resource usage
    pub async fn track_resource_usage(&self) -> Result<ResourceUsage, RepositoryError> {
        let resource_usage = self.get_current_resource_usage().await?;

        // Record individual metrics
        self.metrics_repo
            .record_gauge("resource_cpu_percent", resource_usage.cpu_percentage, &HashMap::new())
            .await?;

        self.metrics_repo
            .record_gauge("resource_memory_mb", resource_usage.memory_mb, &HashMap::new())
            .await?;

        self.metrics_repo
            .record_gauge("resource_disk_mb", resource_usage.disk_usage_mb, &HashMap::new())
            .await?;

        self.metrics_repo
            .record_gauge(
                "resource_network_bytes_per_sec",
                resource_usage.network_bytes_per_sec,
                &HashMap::new(),
            )
            .await?;

        Ok(resource_usage)
    }

    /// Clean up old monitoring data
    pub async fn cleanup_monitoring_data(
        &self,
        retention_days: u32,
    ) -> Result<MonitoringCleanupResult, RepositoryError> {
        let cutoff = Utc::now() - Duration::days(retention_days as i64);

        // Clean up metrics
        let metrics_deleted = self.metrics_repo.cleanup_old_metrics(cutoff).await?;

        // Clean up monitoring data (alerts, health checks, dashboard snapshots)
        let monitoring_deleted = self.monitoring_repo.cleanup_old_data(cutoff).await?;

        // Estimate space freed (rough calculation)
        let total_space_freed_mb = ((metrics_deleted + monitoring_deleted) as f64 * 1.5) / 1024.0;

        Ok(MonitoringCleanupResult {
            metrics_deleted,
            alerts_archived: monitoring_deleted / 3, // Rough estimate
            dashboard_snapshots_deleted: monitoring_deleted / 3, // Rough estimate
            total_space_freed_mb,
        })
    }

    /// Invalidate the CPU metrics cache
    ///
    /// Forces the next call to `get_current_cpu_usage()` to fetch a fresh
    /// measurement regardless of cache age. Useful for:
    /// - Forcing immediate refresh after system changes
    /// - Clearing stale data after long idle periods
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn example(service: &MonitoringService) {
    /// // Clear cache before critical measurement
    /// service.invalidate_cpu_cache().await;
    /// let fresh_cpu = service.get_current_cpu_usage().await?;
    /// # }
    /// ```
    pub async fn invalidate_cpu_cache(&self) {
        let mut cache = self.cpu_cache.last_reading.write().await;
        *cache = None;
        tracing::debug!("CPU cache manually invalidated");
    }

    /// Get the age of the current cached CPU value
    ///
    /// Returns `None` if no cached value exists, otherwise returns the
    /// duration since the value was cached.
    ///
    /// Useful for monitoring and diagnostics.
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn example(service: &MonitoringService) {
    /// if let Some(age) = service.cpu_cache_age().await {
    ///     println!("CPU cache is {} seconds old", age.as_secs());
    /// } else {
    ///     println!("No CPU value cached");
    /// }
    /// # }
    /// ```
    pub async fn cpu_cache_age(&self) -> Option<std::time::Duration> {
        let cache = self.cpu_cache.last_reading.read().await;
        cache.as_ref().map(|reading| reading.timestamp.elapsed())
    }
    
    /// Check if CPU cache is currently valid (not expired)
    ///
    /// Returns `true` if a cached value exists and is within the cache duration.
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn example(service: &MonitoringService) {
    /// if service.is_cpu_cache_valid().await {
    ///     // Next CPU read will use cache (fast)
    /// } else {
    ///     // Next CPU read will spawn blocking task (slow)
    /// }
    /// # }
    /// ```
    pub async fn is_cpu_cache_valid(&self) -> bool {
        self.cpu_cache.get().await.is_some()
    }

    // Private helper methods for system metrics collection

    /// Get current CPU usage percentage
    ///
    /// Uses a 5-second cache to avoid excessive spawn_blocking calls.
    /// CPU measurements require a 200ms delay between readings due to OS-level
    /// requirements (see sysinfo::MINIMUM_CPU_UPDATE_INTERVAL). Without caching,
    /// high-frequency monitoring would exhaust the tokio blocking thread pool.
    ///
    /// # Caching Behavior
    /// - Cache hit (< 5s old): Returns immediately from cache (no blocking)
    /// - Cache miss (≥ 5s old): Spawns blocking task, updates cache, returns fresh value
    ///
    /// # Returns
    /// CPU usage as a percentage (0.0 - 100.0)
    ///
    /// # Errors
    /// Returns `RepositoryError::SystemError` if the blocking task fails
    async fn get_current_cpu_usage(&self) -> Result<f64, RepositoryError> {
        // Check cache first (fast path - uses read lock, allows concurrent access)
        if let Some(cached_value) = self.cpu_cache.get().await {
            return Ok(cached_value);
        }

        // Cache miss - need fresh measurement (slow path - 200ms blocking operation)
        tracing::debug!("CPU cache miss, spawning blocking task for fresh measurement");

        let cpu_usage = tokio::task::spawn_blocking(|| {
            let mut system = System::new();
            system.refresh_cpu_all();

            // Required delay for accurate CPU measurement (OS requirement)
            // See: tmp/sysinfo/src/unix/apple/system.rs - MINIMUM_CPU_UPDATE_INTERVAL
            std::thread::sleep(std::time::Duration::from_millis(200));
            system.refresh_cpu_all();

            system.global_cpu_usage() as f64
        })
        .await
        .map_err(|e| RepositoryError::SystemError(format!("CPU metrics task failed: {}", e)))?;

        // Update cache with fresh value (uses write lock, single writer)
        self.cpu_cache.set(cpu_usage).await;

        Ok(cpu_usage)
    }

    async fn get_current_memory_usage(&self) -> Result<f64, RepositoryError> {
        // Check cache first
        if let Some(cached_value) = self.memory_cache.get().await {
            return Ok(cached_value);
        }

        // Cache miss - need fresh measurement
        tracing::debug!("Memory cache miss, spawning blocking task for fresh measurement");

        let memory_mb = tokio::task::spawn_blocking(|| {
            let mut system = System::new();
            system.refresh_memory();

            (system.used_memory() / 1024 / 1024) as f64
        }).await
        .map_err(|e| RepositoryError::SystemError(format!("Memory metrics task failed: {}", e)))?;

        // Update cache
        self.memory_cache.set(memory_mb).await;

        Ok(memory_mb)
    }

    async fn get_current_disk_usage(&self) -> Result<f64, RepositoryError> {
        // Check cache first
        if let Some(cached_value) = self.disk_cache.get().await {
            return Ok(cached_value);
        }

        // Cache miss - need fresh measurement
        tracing::debug!("Disk cache miss, spawning blocking task for fresh measurement");

        let data_path = std::env::current_dir()
            .map_err(|e| RepositoryError::SystemError(format!("Failed to get current dir: {}", e)))?;

        let disk_usage_mb = tokio::task::spawn_blocking(move || {
            let disks = Disks::new_with_refreshed_list();

            for disk in disks.list() {
                if let Some(mount_point) = disk.mount_point().to_str()
                    && data_path.starts_with(mount_point)
                {
                    let total_mb = (disk.total_space() / 1024 / 1024) as f64;
                    let available_mb = (disk.available_space() / 1024 / 1024) as f64;
                    let used_mb = total_mb - available_mb;
                    return used_mb;
                }
            }

            disks.list().iter()
                .map(|disk| {
                    let total = disk.total_space();
                    let available = disk.available_space();
                    ((total - available) / 1024 / 1024) as f64
                })
                .sum()
        }).await
        .map_err(|e| RepositoryError::SystemError(format!("Disk metrics task failed: {}", e)))?;

        // Update cache
        self.disk_cache.set(disk_usage_mb).await;

        Ok(disk_usage_mb)
    }

    async fn get_network_throughput(&self) -> Result<f64, RepositoryError> {
        let last_stats = self.last_network_stats.clone();

        let throughput = tokio::task::spawn_blocking(move || {
            let networks = Networks::new_with_refreshed_list();

            let now = Instant::now();
            let (total_rx, total_tx): (u64, u64) = networks.list().values().map(|data| {
                    (data.total_received(), data.total_transmitted())
                })
                .fold((0, 0), |(acc_rx, acc_tx), (rx, tx)| {
                    (acc_rx + rx, acc_tx + tx)
                });

            let total_bytes = total_rx + total_tx;

            let mut last = last_stats.blocking_lock();
            let bytes_per_sec = if let Some((prev_time, prev_bytes, _)) = *last {
                let duration_secs = now.duration_since(prev_time).as_secs_f64();
                if duration_secs > 0.0 {
                    (total_bytes.saturating_sub(prev_bytes)) as f64 / duration_secs
                } else {
                    0.0
                }
            } else {
                0.0
            };

            *last = Some((now, total_bytes, total_tx));

            bytes_per_sec
        }).await
        .map_err(|e| RepositoryError::SystemError(format!("Network metrics task failed: {}", e)))?;

        Ok(throughput)
    }

    async fn get_current_resource_usage(&self) -> Result<ResourceUsage, RepositoryError> {
        Ok(ResourceUsage {
            cpu_percentage: self.get_current_cpu_usage().await?,
            memory_mb: self.get_current_memory_usage().await?,
            disk_usage_mb: self.get_current_disk_usage().await?,
            network_bytes_per_sec: self.get_network_throughput().await?,
            timestamp: Utc::now(),
        })
    }

    // Dashboard generation methods

    async fn generate_overview_dashboard(
        &self,
        time_range: &TimeRange,
    ) -> Result<DashboardData, RepositoryError> {
        let summary = self.performance_repo.get_performance_summary(time_range).await?;
        let system_health = self.monitoring_repo.get_system_health().await?;
        let active_alerts = self.monitoring_repo.get_active_alerts().await?;

        let dashboard_data = serde_json::json!({
            "performance_summary": summary,
            "system_health": system_health,
            "active_alerts_count": active_alerts.len(),
            "time_range": time_range
        });

        self.monitoring_repo
            .create_dashboard_snapshot("overview", &dashboard_data)
            .await?;

        Ok(DashboardData {
            id: "overview".to_string(),
            data: dashboard_data,
            last_updated: Utc::now(),
        })
    }

    async fn generate_performance_dashboard(
        &self,
        time_range: &TimeRange,
    ) -> Result<DashboardData, RepositoryError> {
        let summary = self.performance_repo.get_performance_summary(time_range).await?;
        let slow_requests = self.performance_repo.get_slow_requests(1000.0, Some(10)).await?;
        let error_rates = self.performance_repo.get_error_rates(time_range).await?;
        let lazy_loading = self.performance_repo.get_lazy_loading_performance(time_range).await?;

        let dashboard_data = serde_json::json!({
            "performance_summary": summary,
            "slow_requests": slow_requests,
            "error_rates": error_rates,
            "lazy_loading_metrics": lazy_loading,
            "time_range": time_range
        });

        self.monitoring_repo
            .create_dashboard_snapshot("performance", &dashboard_data)
            .await?;

        Ok(DashboardData {
            id: "performance".to_string(),
            data: dashboard_data,
            last_updated: Utc::now(),
        })
    }

    async fn generate_alerts_dashboard(
        &self,
        time_range: &TimeRange,
    ) -> Result<DashboardData, RepositoryError> {
        let active_alerts = self.monitoring_repo.get_active_alerts().await?;
        let critical_alerts = self
            .monitoring_repo
            .get_alerts_by_severity(AlertSeverity::Critical, time_range)
            .await?;
        let warning_alerts = self
            .monitoring_repo
            .get_alerts_by_severity(AlertSeverity::Warning, time_range)
            .await?;

        let dashboard_data = serde_json::json!({
            "active_alerts": active_alerts,
            "critical_alerts_count": critical_alerts.len(),
            "warning_alerts_count": warning_alerts.len(),
            "recent_critical": critical_alerts.into_iter().take(5).collect::<Vec<_>>(),
            "recent_warning": warning_alerts.into_iter().take(5).collect::<Vec<_>>(),
            "time_range": time_range
        });

        self.monitoring_repo
            .create_dashboard_snapshot("alerts", &dashboard_data)
            .await?;

        Ok(DashboardData {
            id: "alerts".to_string(),
            data: dashboard_data,
            last_updated: Utc::now(),
        })
    }
}
