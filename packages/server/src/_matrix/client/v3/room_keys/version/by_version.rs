use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// DELETE /_matrix/client/v3/room_keys/version/{version}
pub async fn delete(Path(_version): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

/// GET /_matrix/client/v3/room_keys/version/{version}
pub async fn get(Path(_version): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "algorithm": "m.megolm_backup.v1.curve25519-aes-sha2",
        "auth_data": {},
        "count": 0,
        "etag": "0",
        "version": _version
    })))
}

/// PUT /_matrix/client/v3/room_keys/version/{version}
pub async fn put(
    Path(_version): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
