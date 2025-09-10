use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// POST /_matrix/client/v3/account/3pid/unbind
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "id_server_unbind_result": "success"
    })))
}
