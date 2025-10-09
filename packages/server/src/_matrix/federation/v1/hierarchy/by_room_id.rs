use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};

use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{
    Room, SpaceHierarchyChildRoomsChunk, SpaceHierarchyParentRoom, SpaceHierarchyResponse,
};
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

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
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
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
    let membership_repo = MembershipRepository::new(state.db.clone());
    let event_repo = EventRepository::new(state.db.clone());

    // Check if the requesting server has any users in the room (current or historical)
    let has_users = membership_repo
        .has_server_users_in_room(&room.room_id, requesting_server)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    if has_users {
        return Ok(true);
    }

    // Check if space is world-readable or publicly joinable
    let world_readable = event_repo
        .get_room_history_visibility(&room.room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        == "world_readable";

    let publicly_joinable = event_repo
        .get_room_join_rules(&room.room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        == "public";

    Ok(world_readable || publicly_joinable)
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
    let (children, inaccessible_children) = get_space_children(state, room_id, requesting_server).await?;

    Ok(SpaceHierarchyResponse {
        room: root_room,
        children,
        inaccessible_children,
    })
}

/// Get room information for hierarchy display
async fn get_room_hierarchy_info(
    state: &AppState,
    room_id: &str,
    _requesting_server: &str,
) -> Result<SpaceHierarchyParentRoom, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = EventRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());

    // Get room metadata from state events
    let event_types = &[
        "m.room.name",
        "m.room.topic",
        "m.room.avatar",
        "m.room.canonical_alias",
        "m.room.join_rules",
        "m.room.history_visibility",
        "m.room.guest_access",
        "m.room.create",
    ];
    let events = event_repo
        .get_room_state_by_types(room_id, event_types)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

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
    let member_count = membership_repo
        .get_member_count(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

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

/// Get child rooms for a space
async fn get_space_children(
    state: &AppState,
    room_id: &str,
    requesting_server: &str,
) -> Result<(Vec<SpaceHierarchyChildRoomsChunk>, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = EventRepository::new(state.db.clone());
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));

    // Get m.space.child events
    let child_events = event_repo
        .get_space_child_events(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let mut children = Vec::new();
    let mut inaccessible_children = Vec::new();

    for child_event in child_events {
        let child_room_id = child_event.state_key.unwrap_or_default();
        
        if child_room_id.is_empty() {
            continue; // Skip invalid state_key
        }

        // Check if child room exists on this server
        let child_room = match room_repo.get_by_id(&child_room_id).await {
            Ok(Some(room)) => room,
            Ok(None) => {
                // Room doesn't exist on this server - exclude entirely per spec
                debug!("Child room {} not found on this server, excluding from hierarchy", child_room_id);
                continue;
            }
            Err(e) => {
                warn!("Failed to query child room {}: {}", child_room_id, e);
                continue; // Exclude on error
            }
        };

        // Check if requesting server has access to this child room
        match check_space_access_permission(state, &child_room, requesting_server).await {
            Ok(true) => {
                // Room is accessible - get full details
                match get_child_room_info(state, &child_room_id, requesting_server).await {
                    Ok(child_info) => {
                        children.push(child_info);
                    }
                    Err(e) => {
                        warn!("Failed to get info for accessible child room {}: {}", child_room_id, e);
                        // Could not get details - exclude entirely
                    }
                }
            }
            Ok(false) => {
                // Room exists but is not accessible - add to inaccessible_children
                debug!("Child room {} is inaccessible to server {}", child_room_id, requesting_server);
                inaccessible_children.push(child_room_id);
            }
            Err(e) => {
                warn!("Failed to check access for child room {}: {}", child_room_id, e);
                // Exclude on error
            }
        }
    }

    Ok((children, inaccessible_children))
}

/// Get child room information for hierarchy display
async fn get_child_room_info(
    state: &AppState,
    room_id: &str,
    _requesting_server: &str,
) -> Result<SpaceHierarchyChildRoomsChunk, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = EventRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());

    // Get room metadata from state events
    let event_types = &[
        "m.room.name",
        "m.room.topic",
        "m.room.avatar",
        "m.room.canonical_alias",
        "m.room.join_rules",
        "m.room.history_visibility",
        "m.room.guest_access",
        "m.room.create",
    ];
    let events = event_repo
        .get_room_state_by_types(room_id, event_types)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

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
    let member_count = membership_repo
        .get_member_count(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

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
