use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// PUT /_matrix/federation/v1/3pid/onbind
pub async fn put(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
