use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/make_leave/{roomId}/{userId}
pub async fn get(
    Path((_room_id, _user_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "event": {
            "type": "m.room.member",
            "content": {
                "membership": "leave"
            },
            "state_key": "@example:example.com",
            "room_id": "!example:example.com",
            "sender": "@example:example.com"
        },
        "room_version": "10"
    })))
}
