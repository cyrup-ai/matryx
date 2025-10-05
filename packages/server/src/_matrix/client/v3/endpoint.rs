use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode};
use chrono::Utc;
use serde_json::{Value, json};

/// POST /_matrix/client/v3/endpoint
///
/// Custom diagnostic endpoint for health checking and server information.
/// This is not part of the Matrix specification, but provides useful
/// diagnostic information for monitoring and debugging.
pub async fn post(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let request_type = payload.get("request_type").and_then(|v| v.as_str()).unwrap_or("health");

    match request_type {
        "health" => {
            // Health check - verify database connectivity
            match state.db.health().await {
                Ok(_) => Ok(Json(json!({
                    "status": "healthy",
                    "timestamp": Utc::now().to_rfc3339(),
                    "server_name": state.homeserver_name,
                    "database": "connected",
                    "version": env!("CARGO_PKG_VERSION")
                }))),
                Err(_) => Ok(Json(json!({
                    "status": "unhealthy",
                    "timestamp": Utc::now().to_rfc3339(),
                    "server_name": state.homeserver_name,
                    "database": "disconnected",
                    "version": env!("CARGO_PKG_VERSION")
                }))),
            }
        },
        "info" => {
            // Server information
            Ok(Json(json!({
                "server_name": state.homeserver_name,
                "version": env!("CARGO_PKG_VERSION"),
                "timestamp": Utc::now().to_rfc3339(),
                "capabilities": {
                    "federation": true,
                    "real_time": true,
                    "live_query": true,
                    "encryption": true
                },
                "database": {
                    "type": "SurrealDB",
                    "real_time": true,
                    "live_query": true
                }
            })))
        },
        "ping" => {
            // Simple ping response
            Ok(Json(json!({
                "pong": true,
                "timestamp": Utc::now().to_rfc3339(),
                "server_name": state.homeserver_name
            })))
        },
        _ => {
            // Unknown request type
            Err(StatusCode::BAD_REQUEST)
        },
    }
}
