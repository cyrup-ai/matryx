use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/rooms/{roomId}/threads
pub async fn get(Path(_room_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "next_token": null,
        "prev_token": null
    })))
}
