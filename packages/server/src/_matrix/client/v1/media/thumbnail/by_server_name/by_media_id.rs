use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}
pub async fn get(
    Path((_server_name, _media_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_type": "image/jpeg",
        "content_disposition": "attachment; filename=thumbnail.jpg"
    })))
}
