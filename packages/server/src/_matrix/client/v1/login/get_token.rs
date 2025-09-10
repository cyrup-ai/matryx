use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v1/login/get_token
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "login_token": "example_login_token_12345",
        "expires_in_ms": 120000
    })))
}
