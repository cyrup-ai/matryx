use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/app/v1/thirdparty/protocols
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

pub mod location;
pub mod protocol;
pub mod user;
