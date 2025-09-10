use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// DELETE /_matrix/client/v3/room_keys/keys
pub async fn delete() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "count": 0,
        "etag": "0"
    })))
}

/// GET /_matrix/client/v3/room_keys/keys
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "rooms": {}
    })))
}

/// PUT /_matrix/client/v3/room_keys/keys
pub async fn put(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "count": 0,
        "etag": "1"
    })))
}

pub mod by_room_id;
