use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/search
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "search_categories": {
            "room_events": {
                "results": [],
                "count": 0,
                "highlights": []
            }
        }
    })))
}
