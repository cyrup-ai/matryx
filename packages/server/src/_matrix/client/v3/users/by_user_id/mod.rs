use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/users/{userId}
pub async fn get(Path(_user_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "user_id": "@example:example.com",
        "display_name": "Example User",
        "avatar_url": null
    })))
}

pub mod by_key_name;
