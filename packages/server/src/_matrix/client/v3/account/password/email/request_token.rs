use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/account/password/email/requestToken
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "sid": "example_session_id",
        "submit_url": "https://example.com/submit_token"
    })))
}
