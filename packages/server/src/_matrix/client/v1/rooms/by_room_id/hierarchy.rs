use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    utils::matrix_identifiers::format_system_user_id,
    AppState,
};

#[derive(Deserialize)]
pub struct HierarchyQuery {
    pub from: Option<String>,
    pub limit: Option<u32>,
    pub max_depth: Option<u32>,
    pub suggested_only: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct SpaceChildEvent {
    pub content: SpaceChildContent,
    pub origin_server_ts: u64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct SpaceChildContent {
    pub order: Option<String>,
    pub suggested: Option<bool>,
    pub via: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SpaceHierarchyRoom {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub avatar_url: Option<String>,
    pub num_joined_members: u64,
    pub room_type: Option<String>,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: String,
    pub children_state: Vec<SpaceChildEvent>,
}

#[derive(Serialize)]
pub struct HierarchyResponse {
    pub rooms: Vec<SpaceHierarchyRoom>,
    pub next_batch: Option<String>,
}

/// GET /_matrix/client/v1/rooms/{roomId}/hierarchy
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Query(query): Query<HierarchyQuery>,
) -> Result<Json<HierarchyResponse>, StatusCode> {
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

    // Verify user can access the root space
    let membership_query = "SELECT membership FROM room_members WHERE room_id = $room_id AND user_id = $user_id";
    let mut membership_params = HashMap::new();
    membership_params.insert("room_id".to_string(), Value::String(room_id.clone()));
    membership_params.insert("user_id".to_string(), Value::String(token_info.user_id.clone()));

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

    // Set defaults
    let max_depth = query.max_depth.unwrap_or(10).min(50); // Limit to prevent abuse
    let limit = query.limit.unwrap_or(100).min(1000);
    let suggested_only = query.suggested_only.unwrap_or(false);

    // Traverse space hierarchy
    let rooms = traverse_space_hierarchy(
        &state,
        &room_id,
        &token_info.user_id,
        max_depth,
        suggested_only,
        limit,
    ).await?;

    Ok(Json(HierarchyResponse {
        rooms,
        next_batch: None, // TODO: Implement pagination
    }))
}

async fn traverse_space_hierarchy(
    state: &AppState,
    root_room_id: &str,
    user_id: &str,
    max_depth: u32,
    suggested_only: bool,
    limit: u32,
) -> Result<Vec<SpaceHierarchyRoom>, StatusCode> {
    let mut visited = HashSet::new();
    let mut rooms = Vec::new();
    let mut queue = vec![(root_room_id.to_string(), 0u32)];

    while let Some((room_id, depth)) = queue.pop() {
        if depth > max_depth || rooms.len() >= limit as usize {
            break;
        }

        if visited.contains(&room_id) {
            continue; // Prevent cycles
        }
        visited.insert(room_id.clone());

        // Get room information
        if let Some(room_info) = get_room_info(state, &room_id, user_id).await? {
            // Get space children
            let children = get_space_children(state, &room_id, suggested_only).await?;
            
            let hierarchy_room = SpaceHierarchyRoom {
                room_id: room_id.clone(),
                name: room_info.name,
                topic: room_info.topic,
                canonical_alias: room_info.canonical_alias,
                avatar_url: room_info.avatar_url,
                num_joined_members: room_info.num_joined_members,
                room_type: room_info.room_type,
                world_readable: room_info.world_readable,
                guest_can_join: room_info.guest_can_join,
                join_rule: room_info.join_rule,
                children_state: children.clone(),
            };

            rooms.push(hierarchy_room);

            // Add children to queue for traversal (depth-first)
            for child in children {
                if !visited.contains(&child.state_key) {
                    queue.push((child.state_key, depth + 1));
                }
            }
        }
    }

    Ok(rooms)
}

#[derive(Debug)]
struct RoomInfo {
    name: Option<String>,
    topic: Option<String>,
    canonical_alias: Option<String>,
    avatar_url: Option<String>,
    num_joined_members: u64,
    room_type: Option<String>,
    world_readable: bool,
    guest_can_join: bool,
    join_rule: String,
}

async fn get_room_info(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<Option<RoomInfo>, StatusCode> {
    // Check if user can access this room
    let access_query = r#"
        SELECT membership FROM room_members 
        WHERE room_id = $room_id AND user_id = $user_id
    "#;
    
    let mut access_params = HashMap::new();
    access_params.insert("room_id".to_string(), Value::String(room_id.to_string()));
    access_params.insert("user_id".to_string(), Value::String(user_id.to_string()));

    let access_result = state.database
        .query(access_query, Some(access_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let has_access = access_result
        .first()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("membership"))
        .and_then(|v| v.as_str())
        .map(|membership| membership == "join" || membership == "invite")
        .unwrap_or(false);

    if !has_access {
        return Ok(None);
    }

    // Get room state and member count
    let room_query = r#"
        SELECT 
            (SELECT content.name FROM room_state WHERE room_id = $room_id AND type = 'm.room.name' AND state_key = '')[0] as name,
            (SELECT content.topic FROM room_state WHERE room_id = $room_id AND type = 'm.room.topic' AND state_key = '')[0] as topic,
            (SELECT content.alias FROM room_state WHERE room_id = $room_id AND type = 'm.room.canonical_alias' AND state_key = '')[0] as canonical_alias,
            (SELECT content.url FROM room_state WHERE room_id = $room_id AND type = 'm.room.avatar' AND state_key = '')[0] as avatar_url,
            (SELECT content.type FROM room_state WHERE room_id = $room_id AND type = 'm.room.create' AND state_key = '')[0] as room_type,
            (SELECT content.history_visibility FROM room_state WHERE room_id = $room_id AND type = 'm.room.history_visibility' AND state_key = '')[0] as history_visibility,
            (SELECT content.guest_access FROM room_state WHERE room_id = $room_id AND type = 'm.room.guest_access' AND state_key = '')[0] as guest_access,
            (SELECT content.join_rule FROM room_state WHERE room_id = $room_id AND type = 'm.room.join_rules' AND state_key = '')[0] as join_rule,
            (SELECT count() FROM room_members WHERE room_id = $room_id AND membership = 'join') as num_joined_members
    "#;

    let mut room_params = HashMap::new();
    room_params.insert("room_id".to_string(), Value::String(room_id.to_string()));

    let room_result = state.database
        .query(room_query, Some(room_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(room_data) = room_result.first().and_then(|rows| rows.first()) {
        let room_info = RoomInfo {
            name: room_data.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()),
            topic: room_data.get("topic").and_then(|v| v.as_str()).map(|s| s.to_string()),
            canonical_alias: room_data.get("canonical_alias").and_then(|v| v.as_str()).map(|s| s.to_string()),
            avatar_url: room_data.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
            num_joined_members: room_data.get("num_joined_members").and_then(|v| v.as_u64()).unwrap_or(0),
            room_type: room_data.get("room_type").and_then(|v| v.as_str()).map(|s| s.to_string()),
            world_readable: room_data.get("history_visibility").and_then(|v| v.as_str()).unwrap_or("shared") == "world_readable",
            guest_can_join: room_data.get("guest_access").and_then(|v| v.as_str()).unwrap_or("forbidden") == "can_join",
            join_rule: room_data.get("join_rule").and_then(|v| v.as_str()).unwrap_or("invite").to_string(),
        };

        Ok(Some(room_info))
    } else {
        Ok(None)
    }
}

async fn get_space_children(
    state: &AppState,
    room_id: &str,
    suggested_only: bool,
) -> Result<Vec<SpaceChildEvent>, StatusCode> {
    let query = if suggested_only {
        r#"
            SELECT * FROM space_children 
            WHERE parent_room_id = $room_id AND suggested = true
            ORDER BY order_value, created_at
        "#
    } else {
        r#"
            SELECT * FROM space_children 
            WHERE parent_room_id = $room_id
            ORDER BY order_value, created_at
        "#
    };

    let mut params = HashMap::new();
    params.insert("room_id".to_string(), Value::String(room_id.to_string()));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut children = Vec::new();

    if let Some(child_rows) = result.first() {
        for child_row in child_rows {
            if let (Some(child_room_id), via) = (
                child_row.get("child_room_id").and_then(|v| v.as_str()),
                child_row.get("via").and_then(|v| v.as_array()).unwrap_or(&Vec::new()),
            ) {
                let via_servers: Vec<String> = via
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();

                let child_event = SpaceChildEvent {
                    content: SpaceChildContent {
                        order: child_row.get("order_value").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        suggested: child_row.get("suggested").and_then(|v| v.as_bool()),
                        via: via_servers,
                    },
                    origin_server_ts: child_row.get("created_at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.timestamp_millis() as u64)
                        .unwrap_or(0),
                    sender: format_system_user_id(),
                    state_key: child_room_id.to_string(),
                    event_type: "m.space.child".to_string(),
                };

                children.push(child_event);
            }
        }
    }

    Ok(children)
}