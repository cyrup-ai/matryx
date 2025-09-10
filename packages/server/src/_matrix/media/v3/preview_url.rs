use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/media/v3/preview_url
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "og:title": "Example Title",
        "og:description": "Example description",
        "og:image": "mxc://example.com/example_image",
        "matrix:image:size": 1024
    })))
}
