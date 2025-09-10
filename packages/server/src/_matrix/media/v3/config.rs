use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/media/v3/config
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "m.upload.size": 50000000
    })))
}
