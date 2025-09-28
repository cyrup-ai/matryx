use crate::config::ServerConfig;
use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/version
///
/// Get the implementation name and version of this homeserver.
pub async fn get() -> Result<Json<Value>, StatusCode> {
    let config = ServerConfig::get().map_err(|e| {
        tracing::error!("Failed to get server config: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let _server_name = format!("{}:{}", config.homeserver_name, config.federation_port);

    Ok(Json(json!({
        "server": {
            "name": config.server_implementation_name,
            "version": config.server_implementation_version
        }
    })))
}
