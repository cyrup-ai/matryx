use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/presence/{userId}/status
pub async fn get(Path(_user_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "presence": "online",
        "last_active_ago": 0,
        "status_msg": null,
        "currently_active": true
    })))
}

/// PUT /_matrix/client/v3/presence/{userId}/status
pub async fn put(
    Path(_user_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
