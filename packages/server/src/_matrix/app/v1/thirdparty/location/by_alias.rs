use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/app/v1/thirdparty/location/{alias}
pub async fn get(Path(_alias): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!([])))
}
