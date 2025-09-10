use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/initial_sync
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "rooms": [],
        "presence": [],
        "account_data": []
    })))
}
