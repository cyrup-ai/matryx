use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/identity/v1/3pid/getValidated3pid
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "medium": "email",
        "address": "example@example.com",
        "validated_at": 1234567890
    })))
}
