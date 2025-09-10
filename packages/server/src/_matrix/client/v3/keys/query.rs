use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/keys/query
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "device_keys": {},
        "failures": {},
        "master_keys": {},
        "self_signing_keys": {},
        "user_signing_keys": {}
    })))
}
