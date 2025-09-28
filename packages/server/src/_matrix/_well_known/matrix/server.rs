use axum::{Json, http::StatusCode};
use serde_json::{Value, json};
use crate::config::ServerConfig;

/// GET /.well-known/matrix/server
///
/// Returns server delegation information for Matrix federation.
/// This tells other homeservers where to connect for federation.
pub async fn get() -> Result<Json<Value>, StatusCode> {
    // Get the server configuration
    let config = ServerConfig::get().map_err(|e| {
        tracing::error!("Failed to get server config: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let server_name = format!("{}:{}", config.homeserver_name, config.federation_port);

    Ok(Json(json!({
        "m.server": server_name
    })))
}