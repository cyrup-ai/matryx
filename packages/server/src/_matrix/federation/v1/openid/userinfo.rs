use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/openid/userinfo
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "sub": "@example:example.com"
    })))
}
