use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/pushrules/global/{kind}/{ruleId}/actions
pub async fn get(
    Path((_kind, _rule_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "actions": []
    })))
}

/// PUT /_matrix/client/v3/pushrules/global/{kind}/{ruleId}/actions
pub async fn put(
    Path((_kind, _rule_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
