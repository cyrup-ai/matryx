use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{
    Room,
    SpaceHierarchyChildRoomsChunk,
    SpaceHierarchyParentRoom,
    SpaceHierarchyResponse,
    StrippedStateEvent,
};
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

/// Matrix X-Matrix authentication header parsed structure
#[derive(Debug, Clone)]
struct XMatrixAuth {
    origin: String,
    key_id: String,
    signature: String,
}

/// Parse X-Matrix authentication header
fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<XMatrixAuth, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if !auth_header.starts_with("X-Matrix ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_params = &auth_header[9..]; // Skip "X-Matrix "

    let mut origin = None;
    let mut key = None;
    let mut signature = None;

    for param in auth_params.split(',') {
        let param = param.trim();

        if let Some((key_name, value)) = param.split_once('=') {
            match key_name.trim() {
                "origin" => {
                    origin = Some(value.trim().to_string());
                },
                "key" => {
                    let key_value = value.trim().trim_matches('"');
                    if let Some(key_id) = key_value.strip_prefix("ed25519:") {
                        key = Some(key_id.to_string());
                    } else {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                },
                "sig" => {
                    signature = Some(value.trim().trim_matches('"').to_string());
                },
                _ => {
                    // Unknown parameter, ignore for forward compatibility
                },
            }
        }
    }

    let origin = origin.ok_or(StatusCode::BAD_REQUEST)?;
    let key_id = key.ok_or(StatusCode::BAD_REQUEST)?;
    let signature = signature.ok_or(StatusCode::BAD_REQUEST)?;

    Ok(XMatrixAuth { origin, key_id, signature })
}

/// Validate Matrix room ID format
fn validate_room_id(room_id: &str) -> Result<(), StatusCode> {
    if !room_id.starts_with('!') || !room_id.contains(':') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// GET /_matrix/federation/v1/hierarchy/{roomId}
///
/// Federation version of the Client-Server GET /hierarchy endpoint.
/// Returns the space hierarchy for a given room, including all child rooms
/// that the requesting server could feasibly peek/join.
pub async fn get(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<SpaceHierarchyResponse>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!("Space hierarchy request - origin: {}, room: {}", x_matrix_auth.origin, room_id);

    // Validate server signature
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            &format!("/_matrix/federation/v1/hierarchy/{}", room_id),
            &[],
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate room ID format
    validate_room_id(&room_id).map_err(|_| {
        warn!("Invalid room ID format: {}", room_id);
        StatusCode::BAD_REQUEST
    })?;

    // Check if room exists and is a space
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let room = room_repo
        .get_by_id(&room_id)
        .await
        .map_err(|e| {
            error!("Failed to query room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Room {} not found", room_id);
            StatusCode::NOT_FOUND
        })?;

    // Check if requesting server has permission to access this space
    let has_permission = check_space_access_permission(&state, &room, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check space access permissions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !has_permission {
        warn!(
            "Server {} not authorized to access space hierarchy for room {}",
            x_matrix_auth.origin, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Get space hierarchy
    let hierarchy = get_space_hierarchy(&state, &room_id, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to retrieve space hierarchy: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Retrieved space hierarchy for room {} for server {}", room_id, x_matrix_auth.origin);

    Ok(Json(hierarchy))
}

/// Check if a server has permission to access a space hierarchy
async fn check_space_access_permission(
    state: &AppState,
    room: &Room,
    requesting_server: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Check if the requesting server has any users in the room (current or historical)
    let query = "
        SELECT COUNT() as count
        FROM membership
        WHERE room_id = $room_id
        AND user_id CONTAINS $server_suffix
        LIMIT 1
    ";

    let server_suffix = format!(":{}", requesting_server);

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room.room_id.clone()))
        .bind(("server_suffix", server_suffix))
        .await?;

    #[derive(serde::Deserialize)]
    struct CountResult {
        count: i64,
    }

    let count_result: Option<CountResult> = response.take(0)?;
    let has_users = count_result.map(|c| c.count > 0).unwrap_or(false);

    if has_users {
        return Ok(true);
    }

    // Check if space is world-readable or publicly joinable
    let world_readable = is_room_world_readable(state, &room.room_id).await?;
    let publicly_joinable = is_room_publicly_joinable(state, &room.room_id).await?;

    Ok(world_readable || publicly_joinable)
}

/// Check if a room is world-readable
async fn is_room_world_readable(
    state: &AppState,
    room_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT content.history_visibility
        FROM event
        WHERE room_id = $room_id
        AND type = 'm.room.history_visibility'
        AND state_key = ''
        ORDER BY depth DESC, origin_server_ts DESC
        LIMIT 1
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct HistoryVisibility {
        history_visibility: Option<String>,
    }

    let visibility: Option<HistoryVisibility> = response.take(0)?;
    let history_visibility = visibility
        .and_then(|v| v.history_visibility)
        .unwrap_or_else(|| "shared".to_string());

    Ok(history_visibility == "world_readable")
}

/// Check if a room is publicly joinable
async fn is_room_publicly_joinable(
    state: &AppState,
    room_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT content.join_rule
        FROM event
        WHERE room_id = $room_id
        AND type = 'm.room.join_rules'
        AND state_key = ''
        ORDER BY depth DESC, origin_server_ts DESC
        LIMIT 1
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct JoinRules {
        join_rule: Option<String>,
    }

    let join_rules: Option<JoinRules> = response.take(0)?;
    let join_rule = join_rules
        .and_then(|j| j.join_rule)
        .unwrap_or_else(|| "invite".to_string());

    Ok(join_rule == "public")
}

/// Get space hierarchy for a room
async fn get_space_hierarchy(
    state: &AppState,
    room_id: &str,
    requesting_server: &str,
) -> Result<SpaceHierarchyResponse, Box<dyn std::error::Error + Send + Sync>> {
    // Get the root room information
    let root_room = get_room_hierarchy_info(state, room_id, requesting_server).await?;

    // Get all child rooms via m.space.child events
    let children = get_space_children(state, room_id, requesting_server).await?;

    Ok(SpaceHierarchyResponse {
        room: root_room,
        children,
        inaccessible_children: vec![], // Empty for now - could be populated with rooms that exist but are not accessible
    })
}

/// Get room information for hierarchy display
async fn get_room_hierarchy_info(
    state: &AppState,
    room_id: &str,
    _requesting_server: &str,
) -> Result<SpaceHierarchyParentRoom, Box<dyn std::error::Error + Send + Sync>> {
    // Get room metadata from state events
    let query = "
        SELECT type, state_key, content
        FROM event
        WHERE room_id = $room_id
        AND type IN ['m.room.name', 'm.room.topic', 'm.room.avatar', 'm.room.canonical_alias', 'm.room.join_rules', 'm.room.history_visibility', 'm.room.guest_access', 'm.room.create']
        AND state_key = ''
        ORDER BY depth DESC, origin_server_ts DESC
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct StateEvent {
        #[serde(rename = "type")]
        event_type: String,
        content: Value,
    }

    let events: Vec<StateEvent> = response.take(0)?;
    let mut room_data = HashMap::new();

    // Process state events to extract room metadata
    for event in events {
        match event.event_type.as_str() {
            "m.room.name" => {
                if let Some(name) = event.content.get("name").and_then(|v| v.as_str()) {
                    room_data.insert("name".to_string(), name.to_string());
                }
            },
            "m.room.topic" => {
                if let Some(topic) = event.content.get("topic").and_then(|v| v.as_str()) {
                    room_data.insert("topic".to_string(), topic.to_string());
                }
            },
            "m.room.avatar" => {
                if let Some(url) = event.content.get("url").and_then(|v| v.as_str()) {
                    room_data.insert("avatar_url".to_string(), url.to_string());
                }
            },
            "m.room.canonical_alias" => {
                if let Some(alias) = event.content.get("alias").and_then(|v| v.as_str()) {
                    room_data.insert("canonical_alias".to_string(), alias.to_string());
                }
            },
            "m.room.join_rules" => {
                if let Some(join_rule) = event.content.get("join_rule").and_then(|v| v.as_str()) {
                    room_data.insert("join_rule".to_string(), join_rule.to_string());
                }
            },
            "m.room.history_visibility" => {
                if let Some(visibility) =
                    event.content.get("history_visibility").and_then(|v| v.as_str())
                {
                    room_data.insert(
                        "world_readable".to_string(),
                        (visibility == "world_readable").to_string(),
                    );
                }
            },
            "m.room.guest_access" => {
                if let Some(guest_access) =
                    event.content.get("guest_access").and_then(|v| v.as_str())
                {
                    room_data.insert(
                        "guest_can_join".to_string(),
                        (guest_access == "can_join").to_string(),
                    );
                }
            },
            "m.room.create" => {
                if let Some(room_type) = event.content.get("type").and_then(|v| v.as_str()) {
                    room_data.insert("room_type".to_string(), room_type.to_string());
                }
            },
            _ => {},
        }
    }

    // Get member count
    let member_count = get_room_member_count(state, room_id).await?;

    Ok(SpaceHierarchyParentRoom {
        room_id: room_id.to_string(),
        canonical_alias: room_data.get("canonical_alias").cloned(),
        guest_can_join: room_data.get("guest_can_join").map(|v| v == "true").unwrap_or(false),
        join_rule: room_data.get("join_rule").cloned(),
        name: room_data.get("name").cloned(),
        num_joined_members: member_count,
        room_type: room_data.get("room_type").cloned(),
        topic: room_data.get("topic").cloned(),
        world_readable: room_data.get("world_readable").map(|v| v == "true").unwrap_or(false),
        avatar_url: room_data.get("avatar_url").cloned(),
        children_state: vec![], // Will be populated by get_space_children
        allowed_room_ids: None,
        encryption: None,
        room_version: None,
    })
}

/// Get member count for a room
async fn get_room_member_count(
    state: &AppState,
    room_id: &str,
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT COUNT() as count
        FROM membership
        WHERE room_id = $room_id
        AND membership = 'join'
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct CountResult {
        count: i64,
    }

    let count_result: Option<CountResult> = response.take(0)?;
    Ok(count_result.map(|c| c.count).unwrap_or(0))
}

/// Get child rooms for a space
async fn get_space_children(
    state: &AppState,
    room_id: &str,
    requesting_server: &str,
) -> Result<Vec<SpaceHierarchyChildRoomsChunk>, Box<dyn std::error::Error + Send + Sync>> {
    // Get m.space.child events
    let query = "
        SELECT state_key, content
        FROM event
        WHERE room_id = $room_id
        AND type = 'm.space.child'
        AND state_key != ''
        ORDER BY depth DESC, origin_server_ts DESC
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct SpaceChildEvent {
        state_key: String,
        content: Value,
    }

    let child_events: Vec<SpaceChildEvent> = response.take(0)?;
    let mut children = Vec::new();

    for child_event in child_events {
        let child_room_id = child_event.state_key;

        // Check if child room exists and is accessible
        if let Ok(child_room_info) =
            get_child_room_info(state, &child_room_id, requesting_server).await
        {
            children.push(child_room_info);
        }
    }

    Ok(children)
}

/// Get child room information for hierarchy display
async fn get_child_room_info(
    state: &AppState,
    room_id: &str,
    _requesting_server: &str,
) -> Result<SpaceHierarchyChildRoomsChunk, Box<dyn std::error::Error + Send + Sync>> {
    // Get room metadata from state events
    let query = "
        SELECT type, state_key, content
        FROM event
        WHERE room_id = $room_id
        AND type IN ['m.room.name', 'm.room.topic', 'm.room.avatar', 'm.room.canonical_alias', 'm.room.join_rules', 'm.room.history_visibility', 'm.room.guest_access', 'm.room.create']
        AND state_key = ''
        ORDER BY depth DESC, origin_server_ts DESC
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct StateEvent {
        #[serde(rename = "type")]
        event_type: String,
        content: Value,
    }

    let events: Vec<StateEvent> = response.take(0)?;
    let mut room_data = HashMap::new();

    // Process state events to extract room metadata
    for event in events {
        match event.event_type.as_str() {
            "m.room.name" => {
                if let Some(name) = event.content.get("name").and_then(|v| v.as_str()) {
                    room_data.insert("name".to_string(), name.to_string());
                }
            },
            "m.room.topic" => {
                if let Some(topic) = event.content.get("topic").and_then(|v| v.as_str()) {
                    room_data.insert("topic".to_string(), topic.to_string());
                }
            },
            "m.room.avatar" => {
                if let Some(url) = event.content.get("url").and_then(|v| v.as_str()) {
                    room_data.insert("avatar_url".to_string(), url.to_string());
                }
            },
            "m.room.canonical_alias" => {
                if let Some(alias) = event.content.get("alias").and_then(|v| v.as_str()) {
                    room_data.insert("canonical_alias".to_string(), alias.to_string());
                }
            },
            "m.room.join_rules" => {
                if let Some(join_rule) = event.content.get("join_rule").and_then(|v| v.as_str()) {
                    room_data.insert("join_rule".to_string(), join_rule.to_string());
                }
            },
            "m.room.history_visibility" => {
                if let Some(visibility) =
                    event.content.get("history_visibility").and_then(|v| v.as_str())
                {
                    room_data.insert(
                        "world_readable".to_string(),
                        (visibility == "world_readable").to_string(),
                    );
                }
            },
            "m.room.guest_access" => {
                if let Some(guest_access) =
                    event.content.get("guest_access").and_then(|v| v.as_str())
                {
                    room_data.insert(
                        "guest_can_join".to_string(),
                        (guest_access == "can_join").to_string(),
                    );
                }
            },
            "m.room.create" => {
                if let Some(room_type) = event.content.get("type").and_then(|v| v.as_str()) {
                    room_data.insert("room_type".to_string(), room_type.to_string());
                }
            },
            _ => {},
        }
    }

    // Get member count
    let member_count = get_room_member_count(state, room_id).await?;

    Ok(SpaceHierarchyChildRoomsChunk {
        room_id: room_id.to_string(),
        canonical_alias: room_data.get("canonical_alias").cloned(),
        guest_can_join: room_data.get("guest_can_join").map(|v| v == "true").unwrap_or(false),
        join_rule: room_data.get("join_rule").cloned(),
        name: room_data.get("name").cloned(),
        num_joined_members: member_count,
        room_type: room_data.get("room_type").cloned(),
        topic: room_data.get("topic").cloned(),
        world_readable: room_data.get("world_readable").map(|v| v == "true").unwrap_or(false),
        avatar_url: room_data.get("avatar_url").cloned(),
        children_state: vec![], // Empty for child rooms
        allowed_room_ids: None,
        encryption: None,
        room_version: None,
    })
}
