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
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct MonitoringService<C: Connection> {
    metrics_repo: MetricsRepository<C>,
    performance_repo: PerformanceRepository<C>,
    monitoring_repo: MonitoringRepository<C>,
    last_network_stats: Arc<Mutex<Option<(Instant, u64, u64)>>>,
}

impl<C: Connection> MonitoringService<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            metrics_repo: MetricsRepository::new(db.clone()),
            performance_repo: PerformanceRepository::new(db.clone()),
            monitoring_repo: MonitoringRepository::new(db),
            last_network_stats: Arc::new(Mutex::new(None)),
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

    // Private helper methods for system metrics collection

    async fn get_current_cpu_usage(&self) -> Result<f64, RepositoryError> {
        let cpu_usage = tokio::task::spawn_blocking(|| {
            let mut system = System::new();
            system.refresh_cpu_all();

            std::thread::sleep(std::time::Duration::from_millis(200));
            system.refresh_cpu_all();

            system.global_cpu_usage() as f64
        }).await
        .map_err(|e| RepositoryError::SystemError(format!("CPU metrics task failed: {}", e)))?;

        Ok(cpu_usage)
    }

    async fn get_current_memory_usage(&self) -> Result<f64, RepositoryError> {
        let memory_mb = tokio::task::spawn_blocking(|| {
            let mut system = System::new();
            system.refresh_memory();

            (system.used_memory() / 1024 / 1024) as f64
        }).await
        .map_err(|e| RepositoryError::SystemError(format!("Memory metrics task failed: {}", e)))?;

        Ok(memory_mb)
    }

    async fn get_current_disk_usage(&self) -> Result<f64, RepositoryError> {
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
