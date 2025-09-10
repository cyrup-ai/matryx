use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/voip/turnServer
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "uris": [
            "turn:turn.example.com:3478?transport=udp",
            "turn:turn.example.com:3478?transport=tcp"
        ],
        "ttl": 86400,
        "username": "example_user",
        "password": "example_password"
    })))
}
