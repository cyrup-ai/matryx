use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/events/{eventId}
pub async fn get(Path(_event_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content": {},
        "event_id": _event_id,
        "origin_server_ts": 1234567890,
        "sender": "@example:localhost",
        "type": "m.room.message"
    })))
}
