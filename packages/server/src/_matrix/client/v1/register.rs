use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

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
    Ok(Json(json!({
        "access_token": "syt_example_token",
        "device_id": "EXAMPLE",
        "home_server": "localhost",
        "user_id": "@example:localhost"
    })))
}
