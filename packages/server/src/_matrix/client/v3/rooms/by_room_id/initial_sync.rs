use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/rooms/{roomId}/initialSync
pub async fn get(Path(_room_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "room_id": "!example_room:example.com",
        "messages": {
            "start": "t1-start_token",
            "end": "t1-end_token",
            "chunk": []
        },
        "state": [],
        "presence": [],
        "account_data": []
    })))
}
