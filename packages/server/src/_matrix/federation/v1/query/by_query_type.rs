use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/query/{queryType}
pub async fn get(Path(_query_type): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
