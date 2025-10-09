use axum::Json;
use serde_json::{Value, json};
use crate::auth::errors::MatrixAuthError;

/// POST /_matrix/client/v3/login/get_token
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, MatrixAuthError> {
    Ok(Json(json!({
        "login_token": "example_login_token",
        "expires_in_ms": 120000
    })))
}
