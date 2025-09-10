use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/identity/v1/query
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "mappings": {}
    })))
}

pub mod by_medium;
