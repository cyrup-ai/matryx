use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use tracing::{debug, error, info, warn};

use crate::state::AppState;

use matryx_surrealdb::repository::{PublicRoomsRepository, RoomRepository};

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
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
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

    // Use helper function to get public rooms with proper filtering
    let filter: Option<PublicRoomsFilter> = None; // No search filter for federation GET
    let public_rooms_response = get_public_rooms(
        &state,
        query.limit,
        query.since,
        &filter,
        query.include_all_networks.unwrap_or(false),
        query.third_party_instance_id,
    )
    .await
    .map_err(|e| {
        error!("Failed to get federation public rooms: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Returned {} public rooms for server {}",
        public_rooms_response.chunk.len(),
        x_matrix_auth.origin
    );

    // Convert to federation response format
    let federation_response = PublicRoomsResponse {
        chunk: public_rooms_response.chunk.into_iter().map(|entry| PublishedRoom {
            room_id: entry.room_id,
            name: entry.name,
            topic: entry.topic,
            avatar_url: entry.avatar_url,
            canonical_alias: entry.canonical_alias,
            num_joined_members: entry.num_joined_members,
            room_type: entry.room_type,
            join_rule: entry.join_rule,
            guest_can_join: entry.guest_can_join,
            world_readable: entry.world_readable,
        }).collect(),
        next_batch: public_rooms_response.next_batch,
        prev_batch: public_rooms_response.prev_batch,
        total_room_count_estimate: public_rooms_response.total_room_count_estimate,
    };

    Ok(Json(serde_json::to_value(federation_response).unwrap_or(json!({}))))
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
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
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

    // Use helper function to get public rooms with proper filtering
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
        error!("Failed to get federation public rooms: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Returned {} filtered public rooms for server {}",
        public_rooms_response.chunk.len(),
        x_matrix_auth.origin
    );

    // Convert to federation response format
    let federation_response = PublicRoomsResponse {
        chunk: public_rooms_response.chunk.into_iter().map(|entry| PublishedRoom {
            room_id: entry.room_id,
            name: entry.name,
            topic: entry.topic,
            avatar_url: entry.avatar_url,
            canonical_alias: entry.canonical_alias,
            num_joined_members: entry.num_joined_members,
            room_type: entry.room_type,
            join_rule: entry.join_rule,
            guest_can_join: entry.guest_can_join,
            world_readable: entry.world_readable,
        }).collect(),
        next_batch: public_rooms_response.next_batch,
        prev_batch: public_rooms_response.prev_batch,
        total_room_count_estimate: public_rooms_response.total_room_count_estimate,
    };

    Ok(Json(serde_json::to_value(federation_response).unwrap_or(json!({}))))
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

/// Get public rooms from the database with pagination and filtering using repository
async fn get_public_rooms(
    state: &AppState,
    limit: Option<u32>,
    since: Option<String>,
    filter: &Option<PublicRoomsFilter>,
    _include_all_networks: bool,
    _third_party_instance_id: Option<String>,
) -> Result<PublicRoomsResponse, Box<dyn std::error::Error + Send + Sync>> {
    let public_rooms_repo = PublicRoomsRepository::new(state.db.clone());
    
    let repo_response = if let Some(search_filter) = filter {
        if let Some(search_term) = &search_filter.generic_search_term {
            if !search_term.trim().is_empty() {
                // Use search functionality
                public_rooms_repo.search_public_rooms(search_term, limit).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            } else {
                // No search term, get regular public rooms
                public_rooms_repo.get_public_rooms(limit, since.as_deref()).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            }
        } else {
            // No search term, get regular public rooms
            public_rooms_repo.get_public_rooms(limit, since.as_deref()).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        }
    } else {
        // No filter, get regular public rooms
        public_rooms_repo.get_public_rooms(limit, since.as_deref()).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
    };

    // Convert repository response to server response format with visibility validation
    let mut published_rooms: Vec<PublishedRoom> = Vec::new();
    
    for entry in repo_response.chunk {
        // Validate room visibility settings using helper function
        match get_room_visibility_settings(state, &entry.room_id).await {
            Ok((join_rule, guest_can_join, world_readable)) => {
                published_rooms.push(PublishedRoom {
                    room_id: entry.room_id,
                    name: entry.name,
                    topic: entry.topic,
                    avatar_url: entry.avatar_url,
                    canonical_alias: entry.canonical_alias,
                    num_joined_members: entry.num_joined_members,
                    room_type: entry.room_type,
                    join_rule: Some(join_rule),
                    guest_can_join,
                    world_readable,
                });
            },
            Err(e) => {
                warn!("Failed to get visibility settings for room {}: {}", entry.room_id, e);
                // Fallback to repository data
                published_rooms.push(PublishedRoom {
                    room_id: entry.room_id,
                    name: entry.name,
                    topic: entry.topic,
                    avatar_url: entry.avatar_url,
                    canonical_alias: entry.canonical_alias,
                    num_joined_members: entry.num_joined_members,
                    room_type: entry.room_type,
                    join_rule: Some(entry.join_rule),
                    guest_can_join: entry.guest_can_join,
                    world_readable: entry.world_readable,
                });
            }
        }
    }

    // Get accurate total room count estimate using helper function
    let total_count_estimate = match get_total_public_rooms_count(state, filter.clone()).await {
        Ok(count) => Some(count),
        Err(e) => {
            warn!("Failed to get total public rooms count: {}", e);
            // Fallback to repository estimate
            repo_response.total_room_count_estimate.map(|c| c as u32)
        }
    };

    Ok(PublicRoomsResponse {
        chunk: published_rooms,
        next_batch: repo_response.next_batch,
        prev_batch: repo_response.prev_batch,
        total_room_count_estimate: total_count_estimate,
    })
}



/// Get room visibility settings from state events using repository
pub async fn get_room_visibility_settings(
    state: &AppState,
    room_id: &str,
) -> Result<(String, bool, bool), Box<dyn std::error::Error + Send + Sync>> {
    let room_repo = std::sync::Arc::new(RoomRepository::new(state.db.clone()));
    let settings = room_repo.get_room_visibility_settings(room_id).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(settings)
}

/// Get total count of public rooms for pagination estimate using repository
pub async fn get_total_public_rooms_count(
    state: &AppState,
    _filter: Option<PublicRoomsFilter>,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let public_rooms_repo = PublicRoomsRepository::new(state.db.clone());
    let count = public_rooms_repo.get_public_rooms_count().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(count as u32)
}


