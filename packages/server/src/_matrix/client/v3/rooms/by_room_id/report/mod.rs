use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/rooms/{roomId}/report
pub async fn post(
    Path(_room_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

pub mod by_event_id;
