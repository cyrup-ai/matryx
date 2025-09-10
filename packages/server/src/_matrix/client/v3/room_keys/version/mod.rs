use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/room_keys/version
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "algorithm": "m.megolm_backup.v1.curve25519-aes-sha2",
        "auth_data": {},
        "count": 0,
        "etag": "0",
        "version": "1"
    })))
}

/// POST /_matrix/client/v3/room_keys/version
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "version": "1"
    })))
}

pub mod by_version;
