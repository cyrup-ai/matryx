use crate::metrics::lazy_loading_metrics::LazyLoadingMetrics;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Performance alerting system for lazy loading degradation detection
pub struct LazyLoadingAlerts {
    alert_config: AlertingConfig,
    notification_sender: Arc<dyn AlertNotificationSender + Send + Sync>,
    alert_state: Arc<RwLock<AlertState>>,
    metrics: Arc<LazyLoadingMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    /// Enable performance degradation alerts
    pub enable_performance_alerts: bool,

    /// Alert thresholds
    pub thresholds: AlertThresholds,

    /// Alert aggregation settings
    pub aggregation: AlertAggregationConfig,

    /// Rate limiting configuration
    pub rate_limiting: RateLimitingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Response time thresholds
    pub response_time: ResponseTimeThresholds,

    /// Error rate thresholds
    pub error_rate: ErrorRateThresholds,

    /// Cache performance thresholds
    pub cache_performance: CachePerformanceThresholds,

    /// Memory usage thresholds
    pub memory_usage: MemoryUsageThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTimeThresholds {
    /// Warning threshold in milliseconds
    pub warning_ms: u64,

    /// Critical threshold in milliseconds
    pub critical_ms: u64,

    /// Percentile to monitor (e.g., 95.0 for P95)
    pub percentile: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRateThresholds {
    /// Warning threshold (0.0 to 1.0)
    pub warning_rate: f64,

    /// Critical threshold (0.0 to 1.0)
    pub critical_rate: f64,

    /// Time window for error rate calculation (minutes)
    pub time_window_minutes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePerformanceThresholds {
    /// Minimum acceptable cache hit ratio
    pub min_hit_ratio_warning: f64,

    /// Critical cache hit ratio threshold
    pub min_hit_ratio_critical: f64,

    /// Maximum acceptable cache miss rate increase
    pub max_miss_rate_increase: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsageThresholds {
    /// Warning threshold in MB
    pub warning_mb: f64,

    /// Critical threshold in MB
    pub critical_mb: f64,

    /// Maximum acceptable memory growth rate (MB/hour)
    pub max_growth_rate_mb_per_hour: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertAggregationConfig {
    /// Time window for alert aggregation (minutes)
    pub aggregation_window_minutes: u64,

    /// Minimum number of incidents before triggering alert
    pub min_incidents_for_alert: u32,

    /// Enable alert deduplication
    pub enable_deduplication: bool,

    /// Deduplication window (minutes)
    pub deduplication_window_minutes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Maximum alerts per hour
    pub max_alerts_per_hour: u32,

    /// Cooldown period between similar alerts (minutes)
    pub cooldown_minutes: u64,

    /// Enable exponential backoff for repeated alerts
    pub enable_exponential_backoff: bool,
}

#[derive(Debug)]
struct AlertState {
    recent_alerts: VecDeque<AlertRecord>,
    performance_history: VecDeque<PerformanceSnapshot>,
    last_alert_times: std::collections::HashMap<AlertType, Instant>,
    alert_counts: std::collections::HashMap<AlertType, u32>,
}

#[derive(Debug, Clone)]
struct AlertRecord {
    alert_type: AlertType,
    severity: AlertSeverity,
    timestamp: Instant,
    message: String,
    metrics: PerformanceSnapshot,
}

#[derive(Debug, Clone)]
struct PerformanceSnapshot {
    timestamp: Instant,
    avg_response_time_ms: u64,
    error_rate: f64,
    cache_hit_ratio: f64,
    memory_usage_mb: f64,
    request_count: u64,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum AlertType {
    ResponseTimeDegradation,
    HighErrorRate,
    CachePerformanceDegradation,
    MemoryUsageHigh,
    MemoryLeakDetected,
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

/// Trait for sending alert notifications
#[async_trait::async_trait]
pub trait AlertNotificationSender {
    async fn send_alert(&self, alert: Alert) -> Result<(), AlertError>;
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub timestamp: Instant,
    pub metrics: PerformanceSnapshot,
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AlertError {
    #[error("Failed to send notification: {0}")]
    NotificationError(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Alert configuration error: {0}")]
    ConfigError(String),
}

impl LazyLoadingAlerts {
    pub fn new(
        alert_config: AlertingConfig,
        notification_sender: Arc<dyn AlertNotificationSender + Send + Sync>,
        metrics: Arc<LazyLoadingMetrics>,
    ) -> Self {
        Self {
            alert_config,
            notification_sender,
            alert_state: Arc::new(RwLock::new(AlertState {
                recent_alerts: VecDeque::new(),
                performance_history: VecDeque::new(),
                last_alert_times: std::collections::HashMap::new(),
                alert_counts: std::collections::HashMap::new(),
            })),
            metrics,
        }
    }

    /// Start the alerting system background task
    pub async fn start_monitoring(&self) -> Result<(), AlertError> {
        if !self.alert_config.enable_performance_alerts {
            return Ok(());
        }

        let alert_config = self.alert_config.clone();
        let notification_sender = Arc::clone(&self.notification_sender);
        let alert_state = Arc::clone(&self.alert_state);
        let metrics = Arc::clone(&self.metrics);

        tokio::spawn(async move {
            let alerts = LazyLoadingAlerts {
                alert_config,
                notification_sender,
                alert_state,
                metrics,
            };

            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                if let Err(e) = alerts.check_performance_thresholds().await {
                    tracing::error!("Error checking performance thresholds: {}", e);
                }
            }
        });

        tracing::info!("Started lazy loading performance alerting system");
        Ok(())
    }

    /// Check all performance thresholds and trigger alerts if necessary
    pub async fn check_performance_thresholds(&self) -> Result<(), AlertError> {
        let performance_summary = self.metrics.get_performance_summary();

        let performance_summary = performance_summary.await;

        let snapshot = PerformanceSnapshot {
            timestamp: Instant::now(),
            avg_response_time_ms: performance_summary.avg_processing_time_us / 1000, // Convert to ms
            error_rate: self.calculate_error_rate().await,
            cache_hit_ratio: performance_summary.cache_hit_ratio,
            memory_usage_mb: performance_summary.estimated_memory_usage_kb as f64 / 1024.0,
            request_count: performance_summary.total_requests,
        };

        // Update performance history
        self.update_performance_history(snapshot.clone()).await;

        // Check each threshold type
        self.check_response_time_thresholds(&snapshot).await?;
        self.check_error_rate_thresholds(&snapshot).await?;
        self.check_cache_performance_thresholds(&snapshot).await?;
        self.check_memory_usage_thresholds(&snapshot).await?;

        Ok(())
    }

    async fn check_response_time_thresholds(
        &self,
        snapshot: &PerformanceSnapshot,
    ) -> Result<(), AlertError> {
        let thresholds = &self.alert_config.thresholds.response_time;

        if snapshot.avg_response_time_ms >= thresholds.critical_ms {
            self.trigger_alert(
                AlertType::ResponseTimeDegradation,
                AlertSeverity::Critical,
                format!(
                    "Critical response time degradation: {}ms (threshold: {}ms)",
                    snapshot.avg_response_time_ms, thresholds.critical_ms
                ),
                snapshot.clone(),
                vec![
                    "Check database query performance".to_string(),
                    "Verify cache hit ratios".to_string(),
                    "Consider scaling resources".to_string(),
                ],
            )
            .await?;
        } else if snapshot.avg_response_time_ms >= thresholds.warning_ms {
            self.trigger_alert(
                AlertType::ResponseTimeDegradation,
                AlertSeverity::Warning,
                format!(
                    "Response time degradation detected: {}ms (threshold: {}ms)",
                    snapshot.avg_response_time_ms, thresholds.warning_ms
                ),
                snapshot.clone(),
                vec![
                    "Monitor performance trends".to_string(),
                    "Check for increased load".to_string(),
                ],
            )
            .await?;
        }

        Ok(())
    }

    async fn check_error_rate_thresholds(
        &self,
        snapshot: &PerformanceSnapshot,
    ) -> Result<(), AlertError> {
        let thresholds = &self.alert_config.thresholds.error_rate;

        if snapshot.error_rate >= thresholds.critical_rate {
            self.trigger_alert(
                AlertType::HighErrorRate,
                AlertSeverity::Critical,
                format!(
                    "Critical error rate: {:.2}% (threshold: {:.2}%)",
                    snapshot.error_rate * 100.0,
                    thresholds.critical_rate * 100.0
                ),
                snapshot.clone(),
                vec![
                    "Check application logs for errors".to_string(),
                    "Verify database connectivity".to_string(),
                    "Consider enabling fallback mode".to_string(),
                ],
            )
            .await?;
        } else if snapshot.error_rate >= thresholds.warning_rate {
            self.trigger_alert(
                AlertType::HighErrorRate,
                AlertSeverity::Warning,
                format!(
                    "Elevated error rate: {:.2}% (threshold: {:.2}%)",
                    snapshot.error_rate * 100.0,
                    thresholds.warning_rate * 100.0
                ),
                snapshot.clone(),
                vec![
                    "Monitor error trends".to_string(),
                    "Review recent deployments".to_string(),
                ],
            )
            .await?;
        }

        Ok(())
    }

    async fn check_cache_performance_thresholds(
        &self,
        snapshot: &PerformanceSnapshot,
    ) -> Result<(), AlertError> {
        let thresholds = &self.alert_config.thresholds.cache_performance;

        if snapshot.cache_hit_ratio <= thresholds.min_hit_ratio_critical {
            self.trigger_alert(
                AlertType::CachePerformanceDegradation,
                AlertSeverity::Critical,
                format!(
                    "Critical cache performance: {:.2}% hit ratio (threshold: {:.2}%)",
                    snapshot.cache_hit_ratio * 100.0,
                    thresholds.min_hit_ratio_critical * 100.0
                ),
                snapshot.clone(),
                vec![
                    "Check cache configuration".to_string(),
                    "Verify cache invalidation logic".to_string(),
                    "Consider increasing cache capacity".to_string(),
                ],
            )
            .await?;
        } else if snapshot.cache_hit_ratio <= thresholds.min_hit_ratio_warning {
            self.trigger_alert(
                AlertType::CachePerformanceDegradation,
                AlertSeverity::Warning,
                format!(
                    "Cache performance degradation: {:.2}% hit ratio (threshold: {:.2}%)",
                    snapshot.cache_hit_ratio * 100.0,
                    thresholds.min_hit_ratio_warning * 100.0
                ),
                snapshot.clone(),
                vec![
                    "Monitor cache metrics".to_string(),
                    "Review cache warming strategy".to_string(),
                ],
            )
            .await?;
        }

        Ok(())
    }

    async fn check_memory_usage_thresholds(
        &self,
        snapshot: &PerformanceSnapshot,
    ) -> Result<(), AlertError> {
        let thresholds = &self.alert_config.thresholds.memory_usage;

        if snapshot.memory_usage_mb >= thresholds.critical_mb {
            self.trigger_alert(
                AlertType::MemoryUsageHigh,
                AlertSeverity::Critical,
                format!(
                    "Critical memory usage: {:.1}MB (threshold: {:.1}MB)",
                    snapshot.memory_usage_mb, thresholds.critical_mb
                ),
                snapshot.clone(),
                vec![
                    "Check for memory leaks".to_string(),
                    "Consider cache eviction policies".to_string(),
                    "Scale memory resources".to_string(),
                ],
            )
            .await?;
        } else if snapshot.memory_usage_mb >= thresholds.warning_mb {
            self.trigger_alert(
                AlertType::MemoryUsageHigh,
                AlertSeverity::Warning,
                format!(
                    "High memory usage: {:.1}MB (threshold: {:.1}MB)",
                    snapshot.memory_usage_mb, thresholds.warning_mb
                ),
                snapshot.clone(),
                vec![
                    "Monitor memory trends".to_string(),
                    "Review cache sizes".to_string(),
                ],
            )
            .await?;
        }

        // Check for memory leaks
        if let Some(growth_rate) = self.calculate_memory_growth_rate().await {
            if growth_rate > thresholds.max_growth_rate_mb_per_hour {
                self.trigger_alert(
                    AlertType::MemoryLeakDetected,
                    AlertSeverity::Critical,
                    format!(
                        "Potential memory leak detected: {:.1}MB/hour growth (threshold: {:.1}MB/hour)",
                        growth_rate,
                        thresholds.max_growth_rate_mb_per_hour
                    ),
                    snapshot.clone(),
                    vec![
                        "Investigate memory leak sources".to_string(),
                        "Check cache cleanup logic".to_string(),
                        "Consider restarting service".to_string(),
                    ],
                ).await?;
            }
        }

        Ok(())
    }

    async fn trigger_alert(
        &self,
        alert_type: AlertType,
        severity: AlertSeverity,
        message: String,
        metrics: PerformanceSnapshot,
        suggested_actions: Vec<String>,
    ) -> Result<(), AlertError> {
        // Check rate limiting
        if !self.should_send_alert(&alert_type).await {
            return Err(AlertError::RateLimitExceeded);
        }

        let alert = Alert {
            alert_type: alert_type.clone(),
            severity: severity.clone(),
            title: self.generate_alert_title(&alert_type, &severity),
            message,
            timestamp: Instant::now(),
            metrics,
            suggested_actions,
        };

        // Send notification
        self.notification_sender.send_alert(alert.clone()).await?;

        // Record alert
        self.record_alert(alert).await;

        tracing::warn!(
            alert_type = ?alert_type,
            severity = ?severity,
            "Triggered lazy loading performance alert"
        );

        Ok(())
    }

    async fn should_send_alert(&self, alert_type: &AlertType) -> bool {
        let state = self.alert_state.read().await;

        // Check cooldown period
        if let Some(last_alert_time) = state.last_alert_times.get(alert_type) {
            let cooldown_duration =
                Duration::from_secs(self.alert_config.rate_limiting.cooldown_minutes * 60);

            if last_alert_time.elapsed() < cooldown_duration {
                return false;
            }
        }

        // Check rate limiting
        let current_hour_alerts = state
            .recent_alerts
            .iter()
            .filter(|alert| {
                alert.alert_type == *alert_type &&
                    alert.timestamp.elapsed() < Duration::from_secs(3600)
            })
            .count() as u32;

        current_hour_alerts < self.alert_config.rate_limiting.max_alerts_per_hour
    }

    async fn record_alert(&self, alert: Alert) {
        let mut state = self.alert_state.write().await;

        // Add to recent alerts
        state.recent_alerts.push_back(AlertRecord {
            alert_type: alert.alert_type.clone(),
            severity: alert.severity,
            timestamp: alert.timestamp,
            message: alert.message,
            metrics: alert.metrics,
        });

        // Update last alert time
        state.last_alert_times.insert(alert.alert_type.clone(), alert.timestamp);

        // Update alert count
        *state.alert_counts.entry(alert.alert_type).or_insert(0) += 1;

        // Cleanup old alerts (keep last 100)
        while state.recent_alerts.len() > 100 {
            state.recent_alerts.pop_front();
        }
    }

    async fn update_performance_history(&self, snapshot: PerformanceSnapshot) {
        let mut state = self.alert_state.write().await;

        state.performance_history.push_back(snapshot);

        // Keep last 24 hours of data (assuming 30-second intervals)
        while state.performance_history.len() > 2880 {
            state.performance_history.pop_front();
        }
    }

    async fn calculate_error_rate(&self) -> f64 {
        // This would integrate with actual error tracking
        // For now, return a placeholder
        0.001 // 0.1% error rate
    }

    async fn calculate_memory_growth_rate(&self) -> Option<f64> {
        let state = self.alert_state.read().await;

        if state.performance_history.len() < 120 {
            // Need at least 1 hour of data
            return None;
        }

        let recent = state.performance_history.back()?;
        let older = state.performance_history.get(state.performance_history.len() - 120)?;

        let time_diff_hours =
            recent.timestamp.duration_since(older.timestamp).as_secs_f64() / 3600.0;
        let memory_diff = recent.memory_usage_mb - older.memory_usage_mb;

        Some(memory_diff / time_diff_hours)
    }

    fn generate_alert_title(&self, alert_type: &AlertType, severity: &AlertSeverity) -> String {
        let severity_str = match severity {
            AlertSeverity::Warning => "Warning",
            AlertSeverity::Critical => "Critical",
        };

        let type_str = match alert_type {
            AlertType::ResponseTimeDegradation => "Response Time Degradation",
            AlertType::HighErrorRate => "High Error Rate",
            AlertType::CachePerformanceDegradation => "Cache Performance Degradation",
            AlertType::MemoryUsageHigh => "High Memory Usage",
            AlertType::MemoryLeakDetected => "Memory Leak Detected",
        };

        format!("[{}] Matrix Lazy Loading: {}", severity_str, type_str)
    }
}

/// Console notification sender for development/testing
pub struct ConsoleNotificationSender;

#[async_trait::async_trait]
impl AlertNotificationSender for ConsoleNotificationSender {
    async fn send_alert(&self, alert: Alert) -> Result<(), AlertError> {
        println!("🚨 ALERT: {}", alert.title);
        println!("   Message: {}", alert.message);
        println!("   Severity: {:?}", alert.severity);
        println!("   Timestamp: {:?}", alert.timestamp);
        println!("   Suggested Actions:");
        for action in &alert.suggested_actions {
            println!("   - {}", action);
        }
        println!();

        Ok(())
    }
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            enable_performance_alerts: true,
            thresholds: AlertThresholds {
                response_time: ResponseTimeThresholds {
                    warning_ms: 100,
                    critical_ms: 200,
                    percentile: 95.0,
                },
                error_rate: ErrorRateThresholds {
                    warning_rate: 0.01,  // 1%
                    critical_rate: 0.05, // 5%
                    time_window_minutes: 5,
                },
                cache_performance: CachePerformanceThresholds {
                    min_hit_ratio_warning: 0.70,  // 70%
                    min_hit_ratio_critical: 0.50, // 50%
                    max_miss_rate_increase: 0.20, // 20%
                },
                memory_usage: MemoryUsageThresholds {
                    warning_mb: 75.0,
                    critical_mb: 100.0,
                    max_growth_rate_mb_per_hour: 10.0,
                },
            },
            aggregation: AlertAggregationConfig {
                aggregation_window_minutes: 5,
                min_incidents_for_alert: 3,
                enable_deduplication: true,
                deduplication_window_minutes: 30,
            },
            rate_limiting: RateLimitingConfig {
                max_alerts_per_hour: 10,
                cooldown_minutes: 15,
                enable_exponential_backoff: true,
            },
        }
    }
}
