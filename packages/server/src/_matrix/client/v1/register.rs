use axum::{Json, http::StatusCode};
use serde_json::{Value, json};
use crate::config::ServerConfig;

/// GET /_matrix/client/v1/register
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "flows": [
            {
                "stages": ["m.login.dummy"]
            }
        ]
    })))
}

/// POST /_matrix/client/v1/register
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    let config = ServerConfig::get();
    Ok(Json(json!({
        "access_token": "syt_example_token",
        "device_id": "EXAMPLE",
        "home_server": config.homeserver_name,
        "user_id": format!("@example:{}", config.homeserver_name)
    })))
}
