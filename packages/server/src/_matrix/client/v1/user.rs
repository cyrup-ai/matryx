use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/user
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "user_id": "@example:example.com"
    })))
}
