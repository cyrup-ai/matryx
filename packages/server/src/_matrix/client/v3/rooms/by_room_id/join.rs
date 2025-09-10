use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/rooms/{roomId}/join
pub async fn post(
    Path(_room_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "room_id": "!example_room:example.com"
    })))
}
