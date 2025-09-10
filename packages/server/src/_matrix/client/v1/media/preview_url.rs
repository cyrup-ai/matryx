use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/media/preview_url
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "og:title": "Example Title",
        "og:description": "Example description",
        "og:image": "https://example.com/image.jpg"
    })))
}
