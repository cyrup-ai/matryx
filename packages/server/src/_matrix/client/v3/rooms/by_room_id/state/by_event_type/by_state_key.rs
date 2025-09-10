use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}
pub async fn get(
    Path((_room_id, _event_type, _state_key)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content": {},
        "type": _event_type,
        "state_key": _state_key
    })))
}

/// PUT /_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}
pub async fn put(
    Path((_room_id, _event_type, _state_key)): Path<(String, String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "event_id": "$example:localhost"
    })))
}
