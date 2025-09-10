use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// DELETE /_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}
pub async fn delete(
    Path((_room_id, _session_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "count": 0,
        "etag": "0"
    })))
}

/// GET /_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}
pub async fn get(
    Path((_room_id, _session_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "first_message_index": 0,
        "forwarded_count": 0,
        "is_verified": true,
        "session_data": {}
    })))
}

/// PUT /_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}
pub async fn put(
    Path((_room_id, _session_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "count": 1,
        "etag": "1"
    })))
}
