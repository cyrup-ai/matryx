use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}
pub async fn put(
    Path((_event_type, _txn_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
