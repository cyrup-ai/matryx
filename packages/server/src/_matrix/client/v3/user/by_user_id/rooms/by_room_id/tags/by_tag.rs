use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// DELETE /_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}
pub async fn delete(
    Path((_user_id, _room_id, _tag)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

/// GET /_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}
pub async fn get(
    Path((_user_id, _room_id, _tag)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "tags": {}
    })))
}

/// PUT /_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}
pub async fn put(
    Path((_user_id, _room_id, _tag)): Path<(String, String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
