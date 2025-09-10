use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// PUT /_matrix/federation/v1/exchange_third_party_invite/{roomId}
pub async fn put(
    Path(_room_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
