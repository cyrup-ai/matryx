use crate::config::ServerConfig;
use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// PUT /_matrix/media/v3/upload/{serverName}/{mediaId}
pub async fn put(
    Path((_server_name, _media_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let config = ServerConfig::get();
    Ok(Json(json!({
        "content_uri": format!("mxc://{}/example", config.homeserver_name)
    })))
}
