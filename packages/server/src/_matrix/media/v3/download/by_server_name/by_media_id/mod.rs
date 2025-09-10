use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/media/v3/download/{serverName}/{mediaId}
pub async fn get(
    Path((_server_name, _media_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_type": "image/jpeg",
        "content_disposition": "inline; filename=example.jpg"
    })))
}

pub mod by_file_name;
