use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/profile/{userId}/displayname
pub async fn get(Path(_user_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "displayname": "Example User"
    })))
}

/// PUT /_matrix/client/v3/profile/{userId}/displayname
pub async fn put(
    Path(_user_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
