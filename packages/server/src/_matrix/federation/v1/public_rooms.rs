use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/publicRooms
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "next_token": null,
        "prev_token": null,
        "total_room_count_estimate": 0
    })))
}

/// POST /_matrix/federation/v1/publicRooms
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "chunk": [],
        "next_token": null,
        "prev_token": null,
        "total_room_count_estimate": 0
    })))
}
