use crate::repository::error::RepositoryError;
use crate::repository::metrics::{
    Alert,
    AlertSeverity,
    ComponentHealth,
    DashboardData,
    HealthStatus,
    SystemHealth,
    TimeRange,
    UptimeEventType,
    UptimeStatistics,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

#[derive(Clone)]
pub struct MonitoringRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> MonitoringRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Create a new alert
    pub async fn create_alert(&self, alert: &Alert) -> Result<String, RepositoryError> {
        let alert_id = uuid::Uuid::new_v4().to_string();
        let mut alert_with_id = alert.clone();
        alert_with_id.id = alert_id.clone();

        let _: Option<Alert> = self.db.create(("alert", &alert_id)).content(alert_with_id).await?;
        Ok(alert_id)
    }

    /// Get all active alerts (not resolved)
    pub async fn get_active_alerts(&self) -> Result<Vec<Alert>, RepositoryError> {
        let query = "SELECT * FROM alert WHERE resolved_at IS NULL ORDER BY created_at DESC";
        let mut result = self.db.query(query).await?;
        let alerts: Vec<Alert> = result.take(0)?;
        Ok(alerts)
    }

    /// Resolve an alert
    pub async fn resolve_alert(
        &self,
        alert_id: &str,
        resolved_by: &str,
    ) -> Result<(), RepositoryError> {
        let query = "UPDATE alert SET resolved_at = $resolved_at, resolved_by = $resolved_by WHERE id = $alert_id";
        let mut result = self
            .db
            .query(query)
            .bind(("resolved_at", Utc::now()))
            .bind(("resolved_by", resolved_by.to_string()))
            .bind(("alert_id", alert_id.to_string()))
            .await?;
        let _: Option<Alert> = result.take(0)?;
        Ok(())
    }

    /// Record a health check for a component
    pub async fn record_health_check(
        &self,
        component: &str,
        status: HealthStatus,
        details: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let health_check = HealthCheck {
            id: uuid::Uuid::new_v4().to_string(),
            component: component.to_string(),
            status,
            details: details.map(|d| d.to_string()),
            timestamp: Utc::now(),
        };

        let _: Option<HealthCheck> = self
            .db
            .create(("health_check", &health_check.id))
            .content(health_check)
            .await?;
        Ok(())
    }

    /// Get overall system health status
    pub async fn get_system_health(&self) -> Result<SystemHealth, RepositoryError> {
        // Get latest health check for each component
        let query = "
            SELECT component, status, details, timestamp 
            FROM health_check 
            WHERE timestamp IN (
                SELECT MAX(timestamp) 
                FROM health_check 
                GROUP BY component
            )
            ORDER BY component
        ";
        let mut result = self.db.query(query).await?;
        let health_checks: Vec<HealthCheck> = result.take(0)?;

        // Build component health map
        let mut components = HashMap::new();
        let mut overall_status = HealthStatus::Healthy;

        for check in health_checks {
            let component_health = ComponentHealth {
                status: check.status.clone(),
                message: check.details,
                last_check: check.timestamp,
            };

            // Update overall status based on worst component status
            match (&overall_status, &check.status) {
                (_, HealthStatus::Unhealthy) => overall_status = HealthStatus::Unhealthy,
                (HealthStatus::Healthy, HealthStatus::Degraded) => {
                    overall_status = HealthStatus::Degraded
                },
                _ => {},
            }

            components.insert(check.component, component_health);
        }

        // Calculate uptime
        let uptime_query = "SELECT timestamp FROM uptime_event WHERE event_type = 'Start' ORDER BY timestamp DESC LIMIT 1";
        let mut uptime_result = self.db.query(uptime_query).await?;
        let last_start: Option<UptimeEvent> = uptime_result.take(0)?;

        let uptime_seconds = if let Some(start_event) = last_start {
            (Utc::now() - start_event.timestamp).num_seconds() as u64
        } else {
            0
        };

        Ok(SystemHealth {
            overall_status,
            components,
            uptime_seconds,
            last_check: Utc::now(),
        })
    }

    /// Record an uptime event (start, stop, restart, etc.)
    pub async fn record_uptime_event(
        &self,
        event_type: UptimeEventType,
        timestamp: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let uptime_event = UptimeEvent {
            id: uuid::Uuid::new_v4().to_string(),
            event_type,
            timestamp,
        };

        let _: Option<UptimeEvent> = self
            .db
            .create(("uptime_event", &uptime_event.id))
            .content(uptime_event)
            .await?;
        Ok(())
    }

    /// Get uptime statistics for a time range
    pub async fn get_uptime_statistics(
        &self,
        time_range: &TimeRange,
    ) -> Result<UptimeStatistics, RepositoryError> {
        // Get all uptime events in the range
        let query = "SELECT * FROM uptime_event WHERE timestamp >= $start AND timestamp <= $end ORDER BY timestamp ASC";
        let mut result = self
            .db
            .query(query)
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let events: Vec<UptimeEvent> = result.take(0)?;

        // Calculate uptime statistics
        let total_duration = (time_range.end - time_range.start).num_seconds() as u64;
        let mut total_downtime = 0u64;
        let mut incident_count = 0u64;
        let mut last_incident: Option<DateTime<Utc>> = None;
        let mut is_down = false;
        let mut downtime_start: Option<DateTime<Utc>> = None;

        for event in events {
            match event.event_type {
                UptimeEventType::Stop | UptimeEventType::Maintenance => {
                    if !is_down {
                        is_down = true;
                        downtime_start = Some(event.timestamp);
                        incident_count += 1;
                        last_incident = Some(event.timestamp);
                    }
                },
                UptimeEventType::Start | UptimeEventType::Restart => {
                    if is_down {
                        if let Some(start) = downtime_start {
                            total_downtime += (event.timestamp - start).num_seconds() as u64;
                        }
                        is_down = false;
                        downtime_start = None;
                    }
                },
            }
        }

        // If still down at the end of the range, count the remaining time
        if is_down
            && let Some(start) = downtime_start {
                total_downtime += (time_range.end - start).num_seconds() as u64;
            }

        let uptime_percentage = if total_duration > 0 {
            ((total_duration - total_downtime) as f64 / total_duration as f64) * 100.0
        } else {
            100.0
        };

        Ok(UptimeStatistics {
            uptime_percentage,
            total_downtime_seconds: total_downtime,
            incident_count,
            last_incident,
        })
    }

    /// Create a dashboard snapshot
    pub async fn create_dashboard_snapshot(
        &self,
        dashboard_id: &str,
        data: &serde_json::Value,
    ) -> Result<(), RepositoryError> {
        let snapshot = DashboardSnapshot {
            id: uuid::Uuid::new_v4().to_string(),
            dashboard_id: dashboard_id.to_string(),
            data: data.clone(),
            timestamp: Utc::now(),
        };

        let _: Option<DashboardSnapshot> = self
            .db
            .create(("dashboard_snapshot", &snapshot.id))
            .content(snapshot)
            .await?;
        Ok(())
    }

    /// Get dashboard data for a specific dashboard and time range
    pub async fn get_dashboard_data(
        &self,
        dashboard_id: &str,
        time_range: &TimeRange,
    ) -> Result<DashboardData, RepositoryError> {
        // Get the latest snapshot within the time range
        let query = "
            SELECT * FROM dashboard_snapshot 
            WHERE dashboard_id = $dashboard_id 
            AND timestamp >= $start 
            AND timestamp <= $end 
            ORDER BY timestamp DESC 
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("dashboard_id", dashboard_id.to_string()))
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let snapshot: Option<DashboardSnapshot> = result.take(0)?;

        if let Some(snapshot) = snapshot {
            Ok(DashboardData {
                id: dashboard_id.to_string(),
                data: snapshot.data,
                last_updated: snapshot.timestamp,
            })
        } else {
            // Return empty dashboard data if no snapshot found
            Ok(DashboardData {
                id: dashboard_id.to_string(),
                data: serde_json::json!({}),
                last_updated: Utc::now(),
            })
        }
    }

    /// Get alerts by severity within a time range
    pub async fn get_alerts_by_severity(
        &self,
        severity: AlertSeverity,
        time_range: &TimeRange,
    ) -> Result<Vec<Alert>, RepositoryError> {
        let query = "
            SELECT * FROM alert 
            WHERE severity = $severity 
            AND created_at >= $start 
            AND created_at <= $end 
            ORDER BY created_at DESC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("severity", severity))
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let alerts: Vec<Alert> = result.take(0)?;
        Ok(alerts)
    }

    /// Get component health history
    pub async fn get_component_health_history(
        &self,
        component: &str,
        time_range: &TimeRange,
    ) -> Result<Vec<HealthCheck>, RepositoryError> {
        let query = "
            SELECT * FROM health_check 
            WHERE component = $component 
            AND timestamp >= $start 
            AND timestamp <= $end 
            ORDER BY timestamp ASC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("component", component.to_string()))
            .bind(("start", time_range.start))
            .bind(("end", time_range.end))
            .await?;
        let health_checks: Vec<HealthCheck> = result.take(0)?;
        Ok(health_checks)
    }

    /// Clean up old monitoring data
    pub async fn cleanup_old_data(&self, cutoff: DateTime<Utc>) -> Result<u64, RepositoryError> {
        let mut total_deleted = 0u64;

        // Clean up old health checks
        let health_query = "DELETE FROM health_check WHERE timestamp < $cutoff";
        let mut result = self.db.query(health_query).bind(("cutoff", cutoff)).await?;
        let health_deleted: Option<u64> = result.take(0)?;
        total_deleted += health_deleted.unwrap_or(0);

        // Clean up old dashboard snapshots
        let dashboard_query = "DELETE FROM dashboard_snapshot WHERE timestamp < $cutoff";
        let mut result = self.db.query(dashboard_query).bind(("cutoff", cutoff)).await?;
        let dashboard_deleted: Option<u64> = result.take(0)?;
        total_deleted += dashboard_deleted.unwrap_or(0);

        // Keep alerts longer, only clean up resolved alerts older than cutoff
        let alert_query =
            "DELETE FROM alert WHERE resolved_at IS NOT NULL AND resolved_at < $cutoff";
        let mut result = self.db.query(alert_query).bind(("cutoff", cutoff)).await?;
        let alert_deleted: Option<u64> = result.take(0)?;
        total_deleted += alert_deleted.unwrap_or(0);

        Ok(total_deleted)
    }
}

// Supporting data structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    id: String,
    component: String,
    status: HealthStatus,
    details: Option<String>,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UptimeEvent {
    id: String,
    event_type: UptimeEventType,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DashboardSnapshot {
    id: String,
    dashboard_id: String,
    data: serde_json::Value,
    timestamp: DateTime<Utc>,
}
