use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::Room;
use matryx_surrealdb::repository::RoomRepository;

/// Query parameters for GET /publicRooms
#[derive(Debug, Deserialize)]
pub struct PublicRoomsQuery {
    /// Whether to include all networks/protocols defined by application services
    include_all_networks: Option<bool>,
    /// Maximum number of rooms to return
    limit: Option<u32>,
    /// Pagination token from previous call
    since: Option<String>,
    /// Specific third-party network/protocol to request
    third_party_instance_id: Option<String>,
}

/// Request body for POST /publicRooms
#[derive(Debug, Deserialize, Serialize)]
pub struct PublicRoomsRequest {
    /// Whether to include all networks/protocols defined by application services
    include_all_networks: Option<bool>,
    /// Maximum number of rooms to return
    limit: Option<u32>,
    /// Pagination token from previous call
    since: Option<String>,
    /// Specific third-party network/protocol to request
    third_party_instance_id: Option<String>,
    /// Filter to apply to the results
    filter: Option<PublicRoomsFilter>,
}

/// Filter for public rooms search
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PublicRoomsFilter {
    /// Generic search term
    generic_search_term: Option<String>,
}

/// A published room in the directory
#[derive(Debug, Serialize)]
pub struct PublishedRoom {
    /// URL for the room's avatar, if one is set
    avatar_url: Option<String>,
    /// Canonical alias of the room, if any
    canonical_alias: Option<String>,
    /// Whether guest users may join the room
    guest_can_join: bool,
    /// The room's join rule
    join_rule: Option<String>,
    /// Name of the room, if any
    name: Option<String>,
    /// Number of members joined to the room
    num_joined_members: u32,
    /// ID of the room
    room_id: String,
    /// Type of room (from m.room.create), if any
    room_type: Option<String>,
    /// Plain text topic of the room
    topic: Option<String>,
    /// Whether the room may be viewed by guest users without joining
    world_readable: bool,
}

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

    // Parse comma-separated key=value pairs
    for param in auth_params.split(',') {
        let param = param.trim();

        if let Some((key_name, value)) = param.split_once('=') {
            match key_name.trim() {
                "origin" => {
                    origin = Some(value.trim().to_string());
                },
                "key" => {
                    // Extract key_id from "ed25519:key_id" format
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

/// GET /_matrix/federation/v1/publicRooms
///
/// Lists the server's published room directory.
pub async fn get(
    State(state): State<AppState>,
    Query(query): Query<PublicRoomsQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "public_rooms GET request - origin: {}, limit: {:?}, since: {:?}",
        x_matrix_auth.origin, query.limit, query.since
    );

    // Validate server signature
    let request_body = format!(
        "include_all_networks={}&limit={}&since={}&third_party_instance_id={}",
        query.include_all_networks.unwrap_or(false),
        query.limit.unwrap_or(0),
        query.since.as_deref().unwrap_or(""),
        query.third_party_instance_id.as_deref().unwrap_or("")
    );

    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            "/publicRooms",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    let public_rooms_response = get_public_rooms(
        &state,
        query.limit,
        query.since,
        &None, // No filter for GET request
        query.include_all_networks.unwrap_or(false),
        query.third_party_instance_id,
    )
    .await
    .map_err(|e| {
        error!("Failed to get public rooms: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Returned {} public rooms for server {}",
        public_rooms_response.chunk.len(),
        x_matrix_auth.origin
    );

    Ok(Json(serde_json::to_value(public_rooms_response).unwrap_or(json!({}))))
}

/// POST /_matrix/federation/v1/publicRooms
///
/// Lists the server's published room directory with an optional filter.
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PublicRoomsRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "public_rooms POST request - origin: {}, limit: {:?}, filter: {:?}",
        x_matrix_auth.origin, payload.limit, payload.filter
    );

    // Validate server signature
    let request_body = serde_json::to_string(&payload).unwrap_or_default();
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "POST",
            "/publicRooms",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    let public_rooms_response = get_public_rooms(
        &state,
        payload.limit,
        payload.since,
        &payload.filter,
        payload.include_all_networks.unwrap_or(false),
        payload.third_party_instance_id,
    )
    .await
    .map_err(|e| {
        error!("Failed to get public rooms: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Returned {} filtered public rooms for server {}",
        public_rooms_response.chunk.len(),
        x_matrix_auth.origin
    );

    Ok(Json(serde_json::to_value(public_rooms_response).unwrap_or(json!({}))))
}
/// Room data with member count from database query
#[derive(serde::Deserialize, Clone)]
struct RoomWithMemberCount {
    room_id: String,
    name: Option<String>,
    topic: Option<String>,
    avatar_url: Option<String>,
    canonical_alias: Option<String>,
    room_version: String,
    room_type: Option<String>,
    created_at: Option<String>,
    member_count: Option<u32>,
}

/// Response structure for public rooms directory
#[derive(Debug, Serialize)]
struct PublicRoomsResponse {
    /// Paginated chunk of published rooms
    chunk: Vec<PublishedRoom>,
    /// Pagination token for next batch
    next_batch: Option<String>,
    /// Pagination token for previous batch
    prev_batch: Option<String>,
    /// Estimate of total number of published rooms
    total_room_count_estimate: Option<u32>,
}

/// Get public rooms from the database with pagination and filtering
async fn get_public_rooms(
    state: &AppState,
    limit: Option<u32>,
    since: Option<String>,
    filter: &Option<PublicRoomsFilter>,
    _include_all_networks: bool,
    _third_party_instance_id: Option<String>,
) -> Result<PublicRoomsResponse, Box<dyn std::error::Error + Send + Sync>> {
    let limit = limit.unwrap_or(100).min(500); // Cap at 500 rooms per request
    let offset = parse_pagination_token(&since).unwrap_or(0);

    // Build query for published rooms
    let mut query_conditions = vec![
        "room.visibility = 'public'".to_string(),
        "room.room_id IS NOT NULL".to_string(),
    ];

    // Add search filter if provided
    if let Some(search_filter) = filter {
        if let Some(search_term) = &search_filter.generic_search_term {
            if !search_term.trim().is_empty() {
                query_conditions.push(format!(
                    "(room.name CONTAINS '{}' OR room.topic CONTAINS '{}' OR room.canonical_alias CONTAINS '{}')",
                    search_term.replace('\'', "''"),
                    search_term.replace('\'', "''"),
                    search_term.replace('\'', "''")
                ));
            }
        }
    }

    let where_clause = if query_conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", query_conditions.join(" AND "))
    };

    // Query for published rooms with member counts
    let query = format!(
        "
        SELECT 
            room.*,
            (SELECT COUNT() FROM membership WHERE room_id = room.room_id AND membership = 'join') as member_count
        FROM room
        {}
        ORDER BY member_count DESC, room.created_at DESC
        LIMIT {} START {}
        ",
        where_clause, limit + 1, offset // Get one extra to check if there's a next page
    );

    let mut response = state.db.query(&query).await?;
    let rooms_data: Vec<RoomWithMemberCount> = response.take(0)?;

    // Check if there are more results
    let has_more = rooms_data.len() > limit as usize;
    let rooms_to_return = if has_more {
        &rooms_data[..limit as usize]
    } else {
        &rooms_data
    };

    // Convert to published room format
    let mut published_rooms = Vec::new();
    for room_data in rooms_to_return {
        let published_room = convert_to_published_room(state, room_data).await?;
        published_rooms.push(published_room);
    }

    // Generate pagination tokens
    let next_batch = if has_more {
        Some(generate_pagination_token(offset + limit))
    } else {
        None
    };

    let prev_batch = if offset > 0 {
        Some(generate_pagination_token(offset.saturating_sub(limit)))
    } else {
        None
    };

    // Get total count estimate
    let total_count = get_total_public_rooms_count(state, filter.as_ref().cloned()).await?;

    Ok(PublicRoomsResponse {
        chunk: published_rooms,
        next_batch,
        prev_batch,
        total_room_count_estimate: Some(total_count),
    })
}

/// Convert room data to published room format
async fn convert_to_published_room(
    state: &AppState,
    room_data: &RoomWithMemberCount,
) -> Result<PublishedRoom, Box<dyn std::error::Error + Send + Sync>> {
    // Get room state information
    let (join_rule, guest_can_join, world_readable) =
        get_room_visibility_settings(state, &room_data.room_id).await?;

    Ok(PublishedRoom {
        room_id: room_data.room_id.clone(),
        name: room_data.name.clone(),
        topic: room_data.topic.clone(),
        avatar_url: room_data.avatar_url.clone(),
        canonical_alias: room_data.canonical_alias.clone(),
        num_joined_members: room_data.member_count.unwrap_or(0),
        room_type: room_data.room_type.clone(),
        join_rule: Some(join_rule),
        guest_can_join,
        world_readable,
    })
}

/// Get room visibility settings from state events
async fn get_room_visibility_settings(
    state: &AppState,
    room_id: &str,
) -> Result<(String, bool, bool), Box<dyn std::error::Error + Send + Sync>> {
    // Query for join rules, guest access, and history visibility
    let query = "
        SELECT type, content
        FROM event
        WHERE room_id = $room_id
        AND type IN ['m.room.join_rules', 'm.room.guest_access', 'm.room.history_visibility']
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

    let state_events: Vec<StateEvent> = response.take(0)?;

    let mut join_rule = "invite".to_string(); // Default
    let mut guest_can_join = false; // Default
    let mut world_readable = false; // Default

    for event in state_events {
        match event.event_type.as_str() {
            "m.room.join_rules" => {
                if let Some(rule) = event.content.get("join_rule").and_then(|v| v.as_str()) {
                    join_rule = rule.to_string();
                }
            },
            "m.room.guest_access" => {
                if let Some(access) = event.content.get("guest_access").and_then(|v| v.as_str()) {
                    guest_can_join = access == "can_join";
                }
            },
            "m.room.history_visibility" => {
                if let Some(visibility) =
                    event.content.get("history_visibility").and_then(|v| v.as_str())
                {
                    world_readable = visibility == "world_readable";
                }
            },
            _ => {},
        }
    }

    Ok((join_rule, guest_can_join, world_readable))
}

/// Get total count of public rooms for pagination estimate
async fn get_total_public_rooms_count(
    state: &AppState,
    filter: Option<PublicRoomsFilter>,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let mut query_conditions = vec!["visibility = 'public'".to_string()];

    // Add search filter if provided
    if let Some(search_filter) = filter {
        if let Some(search_term) = &search_filter.generic_search_term {
            if !search_term.trim().is_empty() {
                query_conditions.push(format!(
                    "(name CONTAINS '{}' OR topic CONTAINS '{}' OR canonical_alias CONTAINS '{}')",
                    search_term.replace('\'', "''"),
                    search_term.replace('\'', "''"),
                    search_term.replace('\'', "''")
                ));
            }
        }
    }

    let where_clause = if query_conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", query_conditions.join(" AND "))
    };

    let query = format!("SELECT COUNT() as count FROM room {}", where_clause);

    let mut response = state.db.query(&query).await?;

    #[derive(serde::Deserialize)]
    struct CountResult {
        count: u32,
    }

    let count_result: Option<CountResult> = response.take(0)?;
    Ok(count_result.map(|c| c.count).unwrap_or(0))
}

/// Parse pagination token to get offset
fn parse_pagination_token(token: &Option<String>) -> Option<u32> {
    token.as_ref()?.parse().ok()
}

/// Generate pagination token from offset
fn generate_pagination_token(offset: u32) -> String {
    offset.to_string()
}
