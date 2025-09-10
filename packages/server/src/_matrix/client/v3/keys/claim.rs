use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/keys/claim
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "one_time_keys": {}
    })))
}
