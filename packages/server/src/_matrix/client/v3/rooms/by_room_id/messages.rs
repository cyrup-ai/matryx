use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/rooms/{roomId}/messages
pub async fn get(Path(_room_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "start": "t0",
        "end": "t1"
    })))
}
