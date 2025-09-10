use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/event_auth/{roomId}/{eventId}
pub async fn get(
    Path((_room_id, _event_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "auth_chain": []
    })))
}
