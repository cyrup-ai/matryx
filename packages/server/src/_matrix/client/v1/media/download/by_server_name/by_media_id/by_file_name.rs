use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}
pub async fn get(
    Path((_server_name, _media_id, _file_name)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_type": "image/jpeg",
        "content_disposition": "attachment; filename=example.jpg"
    })))
}
