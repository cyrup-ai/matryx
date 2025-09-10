use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/pushers
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "pushers": []
    })))
}

/// POST /_matrix/client/v3/pushers/set
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

pub mod set;
