use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}
pub async fn post(
    Path((_room_id, _receipt_type, _event_id)): Path<(String, String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
