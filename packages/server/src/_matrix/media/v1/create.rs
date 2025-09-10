use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/media/v1/create
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_uri": "mxc://localhost/example"
    })))
}
