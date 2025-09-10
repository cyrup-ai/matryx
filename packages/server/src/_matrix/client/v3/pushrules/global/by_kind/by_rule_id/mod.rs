use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// DELETE /_matrix/client/v3/pushrules/global/{kind}/{ruleId}
pub async fn delete(
    Path((_kind, _rule_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

/// GET /_matrix/client/v3/pushrules/global/{kind}/{ruleId}
pub async fn get(
    Path((_kind, _rule_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "actions": [],
        "conditions": [],
        "default": false,
        "enabled": true,
        "rule_id": _rule_id
    })))
}

/// PUT /_matrix/client/v3/pushrules/global/{kind}/{ruleId}
pub async fn put(
    Path((_kind, _rule_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}

pub mod actions;
pub mod enabled;
