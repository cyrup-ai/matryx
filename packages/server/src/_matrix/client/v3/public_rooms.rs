use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::net::SocketAddr;
use tracing::{error, info};

use crate::auth::extract_matrix_auth;
use crate::state::AppState;
use matryx_surrealdb::repository::{PublicRoomsRepository, PublicRoomsFilter as RepoPublicRoomsFilter};

#[derive(Deserialize)]
pub struct PublicRoomsFilter {
    pub limit: Option<u64>,
    pub since: Option<String>,
    pub filter: Option<RoomFilter>,
    pub include_all_known_networks: Option<bool>,
    pub third_party_instance_id: Option<String>,
}

#[derive(Deserialize)]
pub struct RoomFilter {
    pub generic_search_term: Option<String>,
    pub room_types: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct PublicRoomsResponse {
    pub chunk: Vec<PublicRoom>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<u64>,
}

#[derive(Serialize)]
pub struct PublicRoom {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: u64,
    pub avatar_url: Option<String>,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: Option<String>,
    pub room_type: Option<String>,
}

/// GET /_matrix/client/v3/publicRooms
pub async fn get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<PublicRoomsResponse>, StatusCode> {
    // Authentication is optional for public rooms directory
    let _user_id = match extract_matrix_auth(&headers, &state.session_service).await {
        Ok(crate::auth::MatrixAuth::User(user_auth)) => Some(user_auth.user_id),
        Ok(crate::auth::MatrixAuth::Server(_)) => None, // Server auth allowed but no user ID
        Ok(crate::auth::MatrixAuth::Anonymous) => None, // Anonymous access allowed
        Err(_) => None,                                 // Allow anonymous access to public rooms
    };

    info!("Public rooms request from {}", addr);

    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<u64>().ok())
        .unwrap_or(10)
        .min(100); // Cap at 100

    let since = params.get("since").cloned();

    // Use PublicRoomsRepository for room listing
    let public_rooms_repo = PublicRoomsRepository::new(state.db.clone());
    let filter = RepoPublicRoomsFilter {
        limit: Some(limit as u32),
        since,
        server: None,
        include_all_known_networks: None,
        third_party_instance_id: None,
    };

    let public_rooms_response = match public_rooms_repo.get_public_rooms(filter.limit, filter.since.as_deref()).await {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to get public rooms: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Convert to response format
    let chunk: Vec<PublicRoom> = public_rooms_response.chunk
        .into_iter()
        .map(|entry| {
            PublicRoom {
                room_id: entry.room_id,
                name: entry.name,
                topic: entry.topic,
                canonical_alias: entry.canonical_alias,
                num_joined_members: entry.num_joined_members as u64,
                avatar_url: entry.avatar_url,
                world_readable: entry.world_readable,
                guest_can_join: entry.guest_can_join,
                join_rule: Some(entry.join_rule),
                room_type: entry.room_type,
            }
        })
        .collect();

    let total_count = public_rooms_response.total_room_count_estimate.unwrap_or(chunk.len() as u64);

    Ok(Json(PublicRoomsResponse {
        chunk,
        next_batch: public_rooms_response.next_batch,
        prev_batch: public_rooms_response.prev_batch,
        total_room_count_estimate: Some(total_count),
    }))
}

/// POST /_matrix/client/v3/publicRooms
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(filter): Json<PublicRoomsFilter>,
) -> Result<Json<PublicRoomsResponse>, StatusCode> {
    // Authentication is optional for public rooms directory
    let _user_id = match extract_matrix_auth(&headers, &state.session_service).await {
        Ok(crate::auth::MatrixAuth::User(user_auth)) => Some(user_auth.user_id),
        Ok(crate::auth::MatrixAuth::Server(_)) => None, // Server auth allowed but no user ID
        Ok(crate::auth::MatrixAuth::Anonymous) => None, // Anonymous access allowed
        Err(_) => None,                                 // Allow anonymous access to public rooms
    };

    info!("Public rooms search from {} with filter", addr);

    let limit = filter.limit.unwrap_or(10).min(100); // Cap at 100

    // Handle Matrix specification federation parameters
    let include_federated = filter.include_all_known_networks.unwrap_or(false);
    let third_party_filter = filter.third_party_instance_id.as_deref();
    
    info!("Public rooms query - federated: {}, third_party_filter: {:?}", 
          include_federated, third_party_filter);

    // Use PublicRoomsRepository for room search
    let public_rooms_repo = PublicRoomsRepository::new(state.db.clone());
    
    let search_response = if let Some(room_filter) = &filter.filter {
        if let Some(search_term) = &room_filter.generic_search_term {
            // Search with term
            match public_rooms_repo.search_public_rooms(search_term, Some(limit as u32)).await {
                Ok(mut response) => {
                    // Apply room type filtering post-query (Matrix spec: filter by room types like m.space)
                    if let Some(allowed_room_types) = &room_filter.room_types {
                        response.chunk.retain(|room| {
                            if let Some(room_type) = &room.room_type {
                                allowed_room_types.contains(room_type)
                            } else {
                                // Include rooms with no room_type if "" is in allowed_room_types
                                allowed_room_types.contains(&String::new())
                            }
                        });
                    }
                    response
                },
                Err(e) => {
                    error!("Failed to search public rooms: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            }
        } else {
            // No search term, get regular public rooms with filtering
            match public_rooms_repo.get_public_rooms(Some(limit as u32), filter.since.as_deref()).await {
                Ok(mut response) => {
                    // Apply room type filtering post-query
                    if let Some(allowed_room_types) = &room_filter.room_types {
                        response.chunk.retain(|room| {
                            if let Some(room_type) = &room.room_type {
                                allowed_room_types.contains(room_type)
                            } else {
                                allowed_room_types.contains(&String::new())
                            }
                        });
                    }
                    response
                },
                Err(e) => {
                    error!("Failed to get public rooms: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            }
        }
    } else {
        // No filter, get regular public rooms
        match public_rooms_repo.get_public_rooms(Some(limit as u32), filter.since.as_deref()).await {
            Ok(response) => response,
            Err(e) => {
                error!("Failed to get public rooms: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            },
        }
    };

    // Convert to response format
    let chunk: Vec<PublicRoom> = search_response.chunk
        .into_iter()
        .map(|entry| {
            PublicRoom {
                room_id: entry.room_id,
                name: entry.name,
                topic: entry.topic,
                canonical_alias: entry.canonical_alias,
                num_joined_members: entry.num_joined_members as u64,
                avatar_url: entry.avatar_url,
                world_readable: entry.world_readable,
                guest_can_join: entry.guest_can_join,
                join_rule: Some(entry.join_rule),
                room_type: entry.room_type,
            }
        })
        .collect();

    Ok(Json(PublicRoomsResponse {
        chunk,
        next_batch: search_response.next_batch,
        prev_batch: search_response.prev_batch,
        total_room_count_estimate: search_response.total_room_count_estimate,
    }))
}
