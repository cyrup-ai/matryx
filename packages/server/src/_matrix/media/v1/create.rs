use crate::config::ServerConfig;
use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/media/v1/create
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    let config = ServerConfig::get();
    Ok(Json(json!({
        "content_uri": format!("mxc://{}/example", config.homeserver_name)
    })))
}
