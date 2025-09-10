use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/app/v1/thirdparty/user/{userid}
pub async fn get(Path(_userid): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!([])))
}
