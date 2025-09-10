use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/capabilities
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "capabilities": {
            "m.change_password": {
                "enabled": true
            },
            "m.room_versions": {
                "default": "9",
                "available": {
                    "1": "stable",
                    "2": "stable",
                    "3": "stable",
                    "4": "stable",
                    "5": "stable",
                    "6": "stable",
                    "7": "stable",
                    "8": "stable",
                    "9": "stable",
                    "10": "stable"
                }
            }
        }
    })))
}
