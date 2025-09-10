use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// DELETE /_matrix/client/v3/directory/room/{roomAlias}
pub async fn delete(Path(_room_alias): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

/// GET /_matrix/client/v3/directory/room/{roomAlias}
pub async fn get(Path(_room_alias): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "room_id": "!example:localhost",
        "servers": ["localhost"]
    })))
}

/// PUT /_matrix/client/v3/directory/room/{roomAlias}
pub async fn put(
    Path(_room_alias): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
