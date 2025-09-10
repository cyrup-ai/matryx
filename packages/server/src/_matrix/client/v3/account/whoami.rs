use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/account/whoami
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "user_id": "@example:localhost",
        "device_id": "EXAMPLE",
        "is_guest": false
    })))
}
