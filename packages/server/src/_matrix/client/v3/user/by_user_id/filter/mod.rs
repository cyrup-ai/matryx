use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/user/{userId}/filter
pub async fn post(
    Path(_user_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "filter_id": "example_filter_id"
    })))
}

pub mod by_filter_id;
