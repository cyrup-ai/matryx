use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/user/{userId}/rooms/{roomId}/tags
pub async fn get(
    Path((_user_id, _room_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "tags": {}
    })))
}

pub mod by_tag;
