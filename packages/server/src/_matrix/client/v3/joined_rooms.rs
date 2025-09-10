use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/joined_rooms
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "joined_rooms": []
    })))
}
