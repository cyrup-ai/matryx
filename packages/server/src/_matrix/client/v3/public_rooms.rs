use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/publicRooms
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "next_batch": null,
        "prev_batch": null,
        "total_room_count_estimate": 0
    })))
}

/// POST /_matrix/client/v3/publicRooms
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "next_batch": null,
        "prev_batch": null,
        "total_room_count_estimate": 0
    })))
}
