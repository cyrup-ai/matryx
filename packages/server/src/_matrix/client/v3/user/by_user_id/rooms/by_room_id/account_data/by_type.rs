use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/user/{userId}/rooms/{roomId}/account_data/{type}
pub async fn get(
    Path((_user_id, _room_id, _type)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

/// PUT /_matrix/client/v3/user/{userId}/rooms/{roomId}/account_data/{type}
pub async fn put(
    Path((_user_id, _room_id, _type)): Path<(String, String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
