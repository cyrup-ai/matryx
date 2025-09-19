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
use tracing::{error, info, warn};

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
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Query account data from database
    let query = "SELECT content FROM account_data WHERE user_id = $user_id AND data_type = $data_type AND room_id IS NONE";
    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id));
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
    let token_info = state.session_service
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

    // Handle special account data types
    match data_type.as_str() {
        "m.direct" => {
            if let Err(status) = handle_direct_message_data(&user_id, &request.content, &state).await {
                return Err(status);
            }
        },
        "m.ignored_user_list" => {
            if let Err(status) = handle_ignored_users_data(&user_id, &request.content, &state).await {
                return Err(status);
            }
        },
        _ => {
            // Handle generic account data types
        }
    }

    // Generate unique ID for account data
    let account_data_id = Uuid::new_v4().to_string();

    // Update or create account data
    let query = r#"
        UPDATE account_data SET 
            content = $content,
            updated_at = time::now()
        WHERE user_id = $user_id AND data_type = $data_type AND room_id IS NONE
        ELSE CREATE account_data SET
            id = $id,
            user_id = $user_id,
            data_type = $data_type,
            room_id = NONE,
            content = $content,
            created_at = time::now(),
            updated_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("id".to_string(), Value::String(account_data_id));
    params.insert("user_id".to_string(), Value::String(user_id.clone()));
    params.insert("data_type".to_string(), Value::String(data_type));
    params.insert("content".to_string(), request.content);

    state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!("Account data updated for user {} type {}", user_id, data_type);

    Ok(Json(serde_json::json!({})))
}

async fn handle_direct_message_data(
    user_id: &str,
    content: &Value,
    state: &AppState,
) -> Result<(), StatusCode> {
    let dm_data: DirectMessageData = serde_json::from_value(content.clone())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Validate all room IDs exist and user has access
    for (target_user, room_ids) in &dm_data.user_rooms {
        for room_id in room_ids {
            if let Err(_) = validate_dm_room_access(state, user_id, target_user, room_id).await {
                warn!("Invalid DM room access: user {} target {} room {}", user_id, target_user, room_id);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }
    
    // Store in database with special DM indexing
    store_dm_account_data(state, user_id, &dm_data).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    info!("Direct message account data updated for user {}", user_id);
    Ok(())
}

async fn handle_ignored_users_data(
    user_id: &str,
    content: &Value,
    state: &AppState,
) -> Result<(), StatusCode> {
    let ignored_data: IgnoredUserList = serde_json::from_value(content.clone())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Update user's ignore list in database
    update_user_ignore_list(state, user_id, &ignored_data.ignored_users).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Apply filtering to existing rooms/invites
    apply_ignore_filters(state, user_id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Update sync filters for real-time filtering
    update_sync_ignore_filters(state, user_id, &ignored_data.ignored_users).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    info!("Ignored users list updated for user {}", user_id);
    Ok(())
}

async fn validate_dm_room_access(
    state: &AppState,
    user_id: &str,
    target_user: &str,
    room_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if room exists
    let query = "SELECT room_id FROM rooms WHERE room_id = $room_id LIMIT 1";
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .await?;
    
    let rooms: Vec<Value> = result.take(0)?;
    if rooms.is_empty() {
        return Err("Room does not exist".into());
    }

    // Check if user is member of the room
    let query = "
        SELECT content.membership
        FROM room_memberships 
        WHERE room_id = $room_id AND user_id = $user_id
        ORDER BY origin_server_ts DESC
        LIMIT 1
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("user_id", user_id))
        .await?;

    let membership_events: Vec<Value> = result.take(0)?;
    
    if let Some(event) = membership_events.first() {
        if let Some(membership) = event.get("membership").and_then(|v| v.as_str()) {
            if membership == "join" {
                return Ok(());
            }
        }
    }

    Err("User is not a member of the room".into())
}

async fn store_dm_account_data(
    state: &AppState,
    user_id: &str,
    dm_data: &DirectMessageData,
) -> Result<(), Box<dyn std::error::Error>> {
    // Store DM mappings with special indexing for fast lookups
    let query = "
        UPDATE dm_mappings SET 
            user_rooms = $user_rooms,
            updated_at = time::now()
        WHERE user_id = $user_id
        ELSE CREATE dm_mappings SET
            id = rand::uuid(),
            user_id = $user_id,
            user_rooms = $user_rooms,
            created_at = time::now(),
            updated_at = time::now()
    ";
    
    state.db
        .query(query)
        .bind(("user_id", user_id))
        .bind(("user_rooms", serde_json::to_value(&dm_data.user_rooms)?))
        .await?;

    Ok(())
}

async fn update_user_ignore_list(
    state: &AppState,
    user_id: &str,
    ignored_users: &HashMap<String, Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = "
        UPDATE user_ignore_lists SET 
            ignored_users = $ignored_users,
            updated_at = time::now()
        WHERE user_id = $user_id
        ELSE CREATE user_ignore_lists SET
            id = rand::uuid(),
            user_id = $user_id,
            ignored_users = $ignored_users,
            created_at = time::now(),
            updated_at = time::now()
    ";
    
    state.db
        .query(query)
        .bind(("user_id", user_id))
        .bind(("ignored_users", serde_json::to_value(ignored_users)?))
        .await?;

    Ok(())
}

async fn apply_ignore_filters(
    state: &AppState,
    user_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Remove pending invites from ignored users
    let query = "
        DELETE FROM room_invites 
        WHERE target_user = $user_id 
        AND sender IN (
            SELECT array::flatten(object::keys(ignored_users)) 
            FROM user_ignore_lists 
            WHERE user_id = $user_id
        )
    ";
    
    state.db
        .query(query)
        .bind(("user_id", user_id))
        .await?;

    Ok(())
}

async fn update_sync_ignore_filters(
    state: &AppState,
    user_id: &str,
    ignored_users: &HashMap<String, Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Update sync filters to exclude events from ignored users
    let ignored_user_ids: Vec<String> = ignored_users.keys().cloned().collect();
    
    let query = "
        UPDATE user_sync_filters SET 
            ignored_senders = $ignored_senders,
            updated_at = time::now()
        WHERE user_id = $user_id
        ELSE CREATE user_sync_filters SET
            id = rand::uuid(),
            user_id = $user_id,
            ignored_senders = $ignored_senders,
            created_at = time::now(),
            updated_at = time::now()
    ";
    
    state.db
        .query(query)
        .bind(("user_id", user_id))
        .bind(("ignored_senders", serde_json::to_value(ignored_user_ids)?))
        .await?;

    Ok(())
}