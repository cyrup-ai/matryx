use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/users/{userId}/{keyName}
pub async fn get(
    Path((_user_id, _key_name)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "key": "example_value"
    })))
}
