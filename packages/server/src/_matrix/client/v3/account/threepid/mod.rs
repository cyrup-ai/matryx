use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/account/3pid
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "threepids": []
    })))
}

/// POST /_matrix/client/v3/account/3pid
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

pub mod add;
pub mod bind;
pub mod delete;
pub mod email;
pub mod msisdn;
pub mod unbind;
