use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/query/directory
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
