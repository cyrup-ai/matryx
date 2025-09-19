use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    AppState,
};

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
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify user is member of the room
    let membership_query = "SELECT membership FROM room_members WHERE room_id = $room_id AND user_id = $user_id";
    let mut membership_params = HashMap::new();
    membership_params.insert("room_id".to_string(), Value::String(room_id.clone()));
    membership_params.insert("user_id".to_string(), Value::String(user_id.clone()));

    let membership_result = state.database
        .query(membership_query, Some(membership_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_member = membership_result
        .first()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("membership"))
        .and_then(|v| v.as_str())
        .map(|membership| membership == "join" || membership == "invite")
        .unwrap_or(false);

    if !is_member {
        return Err(StatusCode::FORBIDDEN);
    }

    // Query room-scoped account data from database
    let query = "SELECT content FROM account_data WHERE user_id = $user_id AND room_id = $room_id AND data_type = $data_type";
    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id));
    params.insert("room_id".to_string(), Value::String(room_id));
    params.insert("data_type".to_string(), Value::String(data_type));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(data_rows) = result.first() {
        if let Some(data_row) = data_rows.first() {
            if let Some(content) = data_row.get("content") {
                return Ok(Json(content.clone()));
            }
        }
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
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify user is member of the room
    let membership_query = "SELECT membership FROM room_members WHERE room_id = $room_id AND user_id = $user_id";
    let mut membership_params = HashMap::new();
    membership_params.insert("room_id".to_string(), Value::String(room_id.clone()));
    membership_params.insert("user_id".to_string(), Value::String(user_id.clone()));

    let membership_result = state.database
        .query(membership_query, Some(membership_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_member = membership_result
        .first()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("membership"))
        .and_then(|v| v.as_str())
        .map(|membership| membership == "join" || membership == "invite")
        .unwrap_or(false);

    if !is_member {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate data type (Matrix spec: must not be empty)
    if data_type.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Generate unique ID for account data
    let account_data_id = Uuid::new_v4().to_string();

    // Update or create room-scoped account data
    let query = r#"
        UPDATE account_data SET 
            content = $content,
            updated_at = time::now()
        WHERE user_id = $user_id AND room_id = $room_id AND data_type = $data_type
        ELSE CREATE account_data SET
            id = $id,
            user_id = $user_id,
            room_id = $room_id,
            data_type = $data_type,
            content = $content,
            created_at = time::now(),
            updated_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("id".to_string(), Value::String(account_data_id));
    params.insert("user_id".to_string(), Value::String(user_id));
    params.insert("room_id".to_string(), Value::String(room_id));
    params.insert("data_type".to_string(), Value::String(data_type));
    params.insert("content".to_string(), request.content);

    state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({})))
}