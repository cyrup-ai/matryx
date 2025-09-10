use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/state_ids/{roomId}
pub async fn get(Path(_room_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "auth_chain_ids": [],
        "pdu_ids": []
    })))
}
