use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/app/v1/rooms/{roomId}/event/{eventId}
pub async fn get(
    Path((_room_id, _event_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "type": "m.room.message",
        "content": {
            "msgtype": "m.text",
            "body": "Example message"
        },
        "event_id": "$example_event_id:example.com",
        "sender": "@example:example.com",
        "origin_server_ts": 1234567890,
        "room_id": "!example_room:example.com"
    })))
}
