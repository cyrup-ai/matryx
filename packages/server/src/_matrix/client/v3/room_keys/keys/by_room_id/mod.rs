use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// DELETE /_matrix/client/v3/room_keys/keys/{roomId}
pub async fn delete(Path(_room_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "count": 0,
        "etag": "0"
    })))
}

/// GET /_matrix/client/v3/room_keys/keys/{roomId}
pub async fn get(Path(_room_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "sessions": {}
    })))
}

/// PUT /_matrix/client/v3/room_keys/keys/{roomId}
pub async fn put(
    Path(_room_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "count": 0,
        "etag": "1"
    })))
}

pub mod by_session_id;
