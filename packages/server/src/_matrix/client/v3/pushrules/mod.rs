use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/pushrules
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "global": {
            "content": [],
            "override": [],
            "room": [],
            "sender": [],
            "underride": []
        }
    })))
}

pub mod global;
