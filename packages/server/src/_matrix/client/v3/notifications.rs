use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/notifications
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "notifications": [],
        "next_token": null,
        "prev_token": null
    })))
}
