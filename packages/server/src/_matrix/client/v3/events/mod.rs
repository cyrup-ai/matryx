use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/events
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "start": "s0",
        "end": "s1"
    })))
}

pub mod by_event_id;
