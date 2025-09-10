//! GET /.well-known/matrix/client
//! Type: client

use axum::{http::StatusCode, response::Json};
use serde_json::{Value, json};
use std::env;
use tracing::error;

pub async fn get() -> Result<Json<Value>, StatusCode> {
    let homeserver_name = env::var("HOMESERVER_NAME").map_err(|_| {
        error!("HOMESERVER_NAME environment variable not set");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "m.homeserver": {
            "base_url": format!("https://{}", homeserver_name)
        }
    })))
}
