use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/rooms/{roomId}/relations/{eventId}
pub async fn get(
    Path((_room_id, _event_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "next_token": null,
        "prev_token": null
    })))
}

pub mod by_rel_type;
