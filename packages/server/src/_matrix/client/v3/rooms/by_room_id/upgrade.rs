use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    utils::matrix_identifiers::generate_room_id,
    AppState,
};

#[derive(Deserialize)]
pub struct RoomUpgradeRequest {
    pub new_version: String,
}

#[derive(Serialize)]
pub struct RoomUpgradeResponse {
    pub replacement_room: String,
}

/// POST /_matrix/client/v3/rooms/{roomId}/upgrade
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<RoomUpgradeRequest>,
) -> Result<Json<RoomUpgradeResponse>, StatusCode> {
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

    // Validate new version
    if request.new_version.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Verify user has power to upgrade room
    let power_check = validate_upgrade_permissions(&state, &room_id, &token_info.user_id).await?;
    if !power_check {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if room is already upgraded
    let tombstone_query = "SELECT new_room_id FROM room_tombstones WHERE old_room_id = $room_id";
    let mut tombstone_params = HashMap::new();
    tombstone_params.insert("room_id".to_string(), Value::String(room_id.clone()));

    let tombstone_result = state.database
        .query(tombstone_query, Some(tombstone_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if tombstone_result.first().and_then(|rows| rows.first()).is_some() {
        return Err(StatusCode::BAD_REQUEST); // Room already upgraded
    }

    // Create new room with upgraded version
    let new_room_id = create_upgraded_room(&state, &request.new_version, &token_info.user_id).await?;

    // Copy essential state events
    copy_room_state(&state, &room_id, &new_room_id).await?;

    // Send tombstone event in old room
    send_tombstone_event(&state, &room_id, &new_room_id, &token_info.user_id).await?;

    // Record the upgrade
    let record_upgrade_query = r#"
        CREATE room_tombstones SET
            old_room_id = $old_room_id,
            new_room_id = $new_room_id,
            upgrade_version = $upgrade_version,
            created_by = $created_by,
            created_at = time::now()
    "#;

    let mut upgrade_params = HashMap::new();
    upgrade_params.insert("old_room_id".to_string(), Value::String(room_id.clone()));
    upgrade_params.insert("new_room_id".to_string(), Value::String(new_room_id.clone()));
    upgrade_params.insert("upgrade_version".to_string(), Value::String(request.new_version));
    upgrade_params.insert("created_by".to_string(), Value::String(token_info.user_id.clone()));

    state.database
        .query(record_upgrade_query, Some(upgrade_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Invite members to new room
    invite_room_members(&state, &room_id, &new_room_id).await?;

    Ok(Json(RoomUpgradeResponse {
        replacement_room: new_room_id,
    }))
}

async fn validate_upgrade_permissions(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<bool, StatusCode> {
    // Check if user is in room and has sufficient power level
    let power_query = r#"
        SELECT 
            rm.membership,
            (SELECT content.users FROM room_state WHERE room_id = $room_id AND type = 'm.room.power_levels' AND state_key = '')[0] as power_users,
            (SELECT content.events_default FROM room_state WHERE room_id = $room_id AND type = 'm.room.power_levels' AND state_key = '')[0] as events_default,
            (SELECT content.state_default FROM room_state WHERE room_id = $room_id AND type = 'm.room.power_levels' AND state_key = '')[0] as state_default
        FROM room_members rm
        WHERE rm.room_id = $room_id AND rm.user_id = $user_id
    "#;

    let mut params = HashMap::new();
    params.insert("room_id".to_string(), Value::String(room_id.to_string()));
    params.insert("user_id".to_string(), Value::String(user_id.to_string()));

    let result = state.database
        .query(power_query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(power_data) = result.first().and_then(|rows| rows.first()) {
        let membership = power_data.get("membership").and_then(|v| v.as_str()).unwrap_or("");
        
        if membership != "join" {
            return Ok(false);
        }

        // Get user's power level (default 0)
        let user_power = power_data
            .get("power_users")
            .and_then(|v| v.as_object())
            .and_then(|users| users.get(user_id))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Room upgrades typically require admin power (50+) or state_default level
        let state_default = power_data
            .get("state_default")
            .and_then(|v| v.as_i64())
            .unwrap_or(50);

        Ok(user_power >= state_default)
    } else {
        Ok(false)
    }
}

async fn create_upgraded_room(
    state: &AppState,
    new_version: &str,
    creator_id: &str,
) -> Result<String, StatusCode> {
    let new_room_id = generate_room_id();

    // Create basic room structure
    let create_room_query = r#"
        CREATE rooms SET
            room_id = $room_id,
            room_version = $room_version,
            created_by = $creator_id,
            created_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("room_id".to_string(), Value::String(new_room_id.clone()));
    params.insert("room_version".to_string(), Value::String(new_version.to_string()));
    params.insert("creator_id".to_string(), Value::String(creator_id.to_string()));

    state.database
        .query(create_room_query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Add creator as member
    let add_member_query = r#"
        CREATE room_members SET
            room_id = $room_id,
            user_id = $user_id,
            membership = 'join',
            created_at = time::now()
    "#;

    let mut member_params = HashMap::new();
    member_params.insert("room_id".to_string(), Value::String(new_room_id.clone()));
    member_params.insert("user_id".to_string(), Value::String(creator_id.to_string()));

    state.database
        .query(add_member_query, Some(member_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(new_room_id)
}

async fn copy_room_state(
    state: &AppState,
    old_room_id: &str,
    new_room_id: &str,
) -> Result<(), StatusCode> {
    // Copy critical state events: name, topic, avatar, power_levels, etc.
    let state_types = vec![
        "m.room.name",
        "m.room.topic", 
        "m.room.avatar",
        "m.room.power_levels",
        "m.room.join_rules",
        "m.room.history_visibility",
        "m.room.guest_access",
    ];

    for state_type in state_types {
        let copy_query = r#"
            INSERT INTO room_state (room_id, type, state_key, content, sender, created_at)
            SELECT $new_room_id, type, state_key, content, sender, time::now()
            FROM room_state 
            WHERE room_id = $old_room_id AND type = $state_type AND state_key = ''
        "#;

        let mut params = HashMap::new();
        params.insert("old_room_id".to_string(), Value::String(old_room_id.to_string()));
        params.insert("new_room_id".to_string(), Value::String(new_room_id.to_string()));
        params.insert("state_type".to_string(), Value::String(state_type.to_string()));

        let _ = state.database
            .query(copy_query, Some(params))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(())
}

async fn send_tombstone_event(
    state: &AppState,
    old_room_id: &str,
    new_room_id: &str,
    sender_id: &str,
) -> Result<(), StatusCode> {
    let tombstone_content = serde_json::json!({
        "body": "This room has been replaced",
        "replacement_room": new_room_id
    });

    let tombstone_query = r#"
        CREATE room_state SET
            room_id = $room_id,
            type = 'm.room.tombstone',
            state_key = '',
            content = $content,
            sender = $sender,
            created_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("room_id".to_string(), Value::String(old_room_id.to_string()));
    params.insert("content".to_string(), tombstone_content);
    params.insert("sender".to_string(), Value::String(sender_id.to_string()));

    state.database
        .query(tombstone_query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

async fn invite_room_members(
    state: &AppState,
    old_room_id: &str,
    new_room_id: &str,
) -> Result<(), StatusCode> {
    // Get all joined members from old room
    let members_query = "SELECT user_id FROM room_members WHERE room_id = $room_id AND membership = 'join'";
    let mut params = HashMap::new();
    params.insert("room_id".to_string(), Value::String(old_room_id.to_string()));

    let result = state.database
        .query(members_query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(member_rows) = result.first() {
        for member_row in member_rows {
            if let Some(user_id) = member_row.get("user_id").and_then(|v| v.as_str()) {
                // Create invitation
                let invite_query = r#"
                    CREATE room_members SET
                        room_id = $room_id,
                        user_id = $user_id,
                        membership = 'invite',
                        created_at = time::now()
                "#;

                let mut invite_params = HashMap::new();
                invite_params.insert("room_id".to_string(), Value::String(new_room_id.to_string()));
                invite_params.insert("user_id".to_string(), Value::String(user_id.to_string()));

                let _ = state.database
                    .query(invite_query, Some(invite_params))
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
    }

    Ok(())
}