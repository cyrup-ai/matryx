use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/user/{userId}/openid/request_token
pub async fn post(
    Path(_user_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "access_token": "example_openid_token",
        "token_type": "Bearer",
        "matrix_server_name": "example.com",
        "expires_in": 3600
    })))
}
