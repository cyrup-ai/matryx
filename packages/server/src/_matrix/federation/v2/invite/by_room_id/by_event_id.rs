use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// PUT /_matrix/federation/v2/invite/{roomId}/{eventId}
pub async fn put(
    Path((_room_id, _event_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "event": {
            "type": "m.room.member",
            "state_key": "@invited:example.com",
            "content": {
                "membership": "invite"
            }
        }
    })))
}
