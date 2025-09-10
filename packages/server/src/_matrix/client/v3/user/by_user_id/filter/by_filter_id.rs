use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/user/{userId}/filter/{filterId}
pub async fn get(
    Path((_user_id, _filter_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "event_fields": [],
        "event_format": "client",
        "presence": {
            "limit": 10
        },
        "account_data": {
            "limit": 10
        },
        "room": {
            "rooms": [],
            "not_rooms": [],
            "ephemeral": {
                "limit": 10
            },
            "include_leave": false,
            "state": {
                "limit": 10
            },
            "timeline": {
                "limit": 10
            },
            "account_data": {
                "limit": 10
            }
        }
    })))
}

/// POST /_matrix/client/v3/user/{userId}/filter
pub async fn post(
    Path(_user_id): Path<String>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "filter_id": "example_filter_id"
    })))
}
