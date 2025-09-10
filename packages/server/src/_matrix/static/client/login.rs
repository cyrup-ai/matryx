use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/static/client/login
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "flows": [
            {
                "type": "m.login.password"
            }
        ]
    })))
}
