use axum::{Json, http::StatusCode};
use serde_json::{Value, json};
use std::env;

/// GET /_matrix/federation/v1/version
///
/// Get the implementation name and version of this homeserver.
pub async fn get() -> Result<Json<Value>, StatusCode> {
    let _server_name = env::var("HOMESERVER_NAME").unwrap_or_else(|_| "localhost:8008".to_string());

    Ok(Json(json!({
        "server": {
            "name": "matryx",
            "version": "0.1.0"
        }
    })))
}
