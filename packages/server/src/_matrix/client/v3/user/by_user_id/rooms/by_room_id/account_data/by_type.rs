use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;


use crate::AppState;
use matryx_surrealdb::repository::{ProfileManagementService, AccountDataRepository};

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

pub async fn get_room_account_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, room_id, data_type)): Path<(String, String, String)>,
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

    // Get room account data using ProfileManagementService
    match profile_service
        .get_account_data(&user_id, &data_type, Some(&room_id))
        .await
    {
        Ok(Some(content)) => return Ok(Json(content)),
        Ok(None) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    // If no account data exists, return 404
    Err(StatusCode::NOT_FOUND)
}

pub async fn set_room_account_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, room_id, data_type)): Path<(String, String, String)>,
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

    // Verify user is member of the room
    let account_data_repo = AccountDataRepository::new(state.db.clone());
    let is_member = account_data_repo
        .check_room_membership(&room_id, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !is_member {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate data type (Matrix spec: must not be empty)
    if data_type.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Create AccountData struct for room-specific account data tracking
    let now = Utc::now();
    let account_data = AccountData {
        id: format!("{}:{}:{}", user_id, room_id, data_type),
        user_id: user_id.clone(),
        room_id: Some(room_id.clone()), // Room-specific account data has room_id
        data_type: data_type.clone(),
        content: request.content.clone(),
        created_at: now,
        updated_at: now,
    };

    tracing::info!("Setting room account data: {:?}", account_data);

    // Set room account data using ProfileManagementService
    match profile_service
        .set_account_data(&user_id, &data_type, request.content, Some(&room_id))
        .await
    {
        Ok(()) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    Ok(Json(serde_json::json!({})))
}

// HTTP method handlers for main.rs routing
pub use get_room_account_data as get;
pub use set_room_account_data as put;
