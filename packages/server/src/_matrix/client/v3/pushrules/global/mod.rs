use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/pushrules/global/
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content": [],
        "override": [],
        "room": [],
        "sender": [],
        "underride": []
    })))
}

pub mod by_kind;
