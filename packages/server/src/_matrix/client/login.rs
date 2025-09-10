use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/login
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "user_id": "@example:example.com",
        "access_token": "example_access_token",
        "device_id": "example_device_id",
        "well_known": {
            "m.homeserver": {
                "base_url": "https://example.com"
            }
        }
    })))
}
