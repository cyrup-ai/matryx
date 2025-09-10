use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/thirdparty/location
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!([])))
}

pub mod by_alias;
pub mod by_protocol;
