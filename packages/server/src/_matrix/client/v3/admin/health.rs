use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode};
use chrono::Utc;
use serde_json::{Value, json};
use matryx_surrealdb::repository::metrics::HealthStatus;
use matryx_surrealdb::repository::monitoring::MonitoringRepository;

/// GET /_matrix/client/v3/admin/health
///
/// Matrix admin health check endpoint providing comprehensive system health status.
/// Returns detailed health information for all system components.
pub async fn get(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    // Get comprehensive health status
    let app_health = state.health_check().await;
    
    // Get detailed database health
    let database_health = match state.database_health_repo.comprehensive_health_check().await {
        Ok(health) => json!({
            "status": match health.status {
                HealthStatus::Healthy => "healthy",
                HealthStatus::Degraded => "degraded", 
                HealthStatus::Unhealthy => "unhealthy",
            },
            "response_time_ms": health.response_time_ms,
            "last_check": health.last_check.to_rfc3339(),
            "details": health.details
        }),
        Err(e) => json!({
            "status": "unhealthy",
            "error": e.to_string(),
            "last_check": Utc::now().to_rfc3339()
        })
    };

    // Determine overall system status
    let overall_status = if app_health.database_connected {
        if app_health.lazy_loading.as_ref().map_or(true, |h| h.is_healthy()) &&
           app_health.memory.as_ref().map_or(true, |h| h.is_healthy()) {
            "healthy"
        } else {
            "degraded"
        }
    } else {
        "unhealthy"
    };

    Ok(Json(json!({
        "status": overall_status,
        "timestamp": Utc::now().to_rfc3339(),
        "server_name": state.homeserver_name,
        "version": env!("CARGO_PKG_VERSION"),
        "components": {
            "database": database_health,
            "lazy_loading": app_health.lazy_loading,
            "memory": app_health.memory
        },
        "uptime_seconds": 0, // TODO: Add actual uptime tracking
    })))
}

/// POST /_matrix/client/v3/admin/health
///
/// Trigger manual health check and optionally record results to monitoring system.
pub async fn post(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let record_to_monitoring = payload.get("record")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Perform health check
    let database_health = state.database_health_repo.comprehensive_health_check().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Optionally record to monitoring system
    if record_to_monitoring {
        // Use existing MonitoringRepository to record health check
        let monitoring_repo = MonitoringRepository::new(state.db.clone());
        let _ = monitoring_repo.record_health_check(
            "database",
            database_health.status.clone(),
            database_health.details.as_deref()
        ).await;
    }

    Ok(Json(json!({
        "status": match database_health.status {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        },
        "timestamp": Utc::now().to_rfc3339(),
        "response_time_ms": database_health.response_time_ms,
        "recorded_to_monitoring": record_to_monitoring
    })))
}