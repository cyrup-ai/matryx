use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// PUT /_matrix/client/v3/rooms/{roomId}/redact/{eventId}
pub async fn put(
    Path((_room_id, _event_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "event_id": "$redacted_event_id:example.com"
    })))
}

pub mod by_txn_id;
