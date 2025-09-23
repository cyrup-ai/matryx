use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{AppState, auth::MatrixSessionService};
use matryx_surrealdb::repository::ProfileManagementService;

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountData {
    pub id: String,
    pub user_id: String,
    pub room_id: Option<String>,
    pub data_type: String,
    pub content: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct SetAccountDataRequest {
    #[serde(flatten)]
    pub content: Value,
}

#[derive(Serialize, Deserialize)]
pub struct DirectMessageData {
    #[serde(flatten)]
    pub user_rooms: HashMap<String, Vec<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct IgnoredUserList {
    pub ignored_users: HashMap<String, Value>,
}

pub async fn get_account_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, data_type)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Get account data using ProfileManagementService
    match profile_service.get_account_data(&user_id, &data_type, None).await {
        Ok(Some(content)) => return Ok(Json(content)),
        Ok(None) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    // If no account data exists, return 404
    Err(StatusCode::NOT_FOUND)
}

pub async fn set_account_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, data_type)): Path<(String, String)>,
    Json(request): Json<SetAccountDataRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate data type (Matrix spec: must not be empty)
    if data_type.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check for server-managed types that should be rejected (Matrix spec)
    if matches!(data_type.as_str(), "m.fully_read" | "m.push_rules") {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }

    // Note: Special handling for m.direct and m.ignored_user_list types
    // has been simplified for repository pattern migration

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Set account data using ProfileManagementService
    match profile_service
        .set_account_data(&user_id, &data_type, request.content, None)
        .await
    {
        Ok(()) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    info!("Account data updated for user {} type {}", user_id, data_type);

    Ok(Json(serde_json::json!({})))
}

// HTTP method handlers for main.rs routing
pub use get_account_data as get;
pub use set_account_data as put;
