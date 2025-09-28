use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
// Note: Using local HierarchyResponse for this endpoint

#[derive(Deserialize)]
pub struct HierarchyQuery {
    pub from: Option<String>,
    pub limit: Option<u32>,
    pub max_depth: Option<u32>,
    pub suggested_only: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SpaceChildEvent {
    pub content: SpaceChildContent,
    pub origin_server_ts: u64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Serialize, Deserialize, Clone)]
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
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room hierarchy request failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room hierarchy request failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            // Server-to-server requests are allowed for federation
            "server".to_string()
        },
        MatrixAuth::Anonymous => {
            warn!("Room hierarchy request failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room hierarchy request for room: {} by user: {}", room_id, user_id);

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room hierarchy request failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Apply pagination parameters from query
    let limit = query.limit.unwrap_or(50).min(1000); // Default 50, max 1000 per Matrix spec
    let from_token = query.from.as_deref(); // Pagination token for continuing from previous request
    
    // TODO: Implement proper pagination support in room hierarchy
    // For now, we validate the parameters but don't use them in the actual query
    if from_token.is_some() {
        info!("Room hierarchy pagination requested from token: {:?}", from_token);
    }
    if limit != 50 {
        info!("Room hierarchy limit override requested: {}", limit);
    }
    
    // Use RoomOperationsService to get room hierarchy with all validation
    match state
        .room_operations
        .get_room_hierarchy(
            &room_id,
            &user_id,
            query.suggested_only.unwrap_or(false),
            query.max_depth,
        )
        .await
    {
        Ok(surreal_hierarchy) => {
            info!("Successfully retrieved room hierarchy for room {}", room_id);

            // Convert surrealdb HierarchyResponse to server HierarchyResponse
            let mut rooms = Vec::new();
            
            // Add the root room first
            let root_room = SpaceHierarchyRoom {
                room_id: surreal_hierarchy.room.room_id.clone(),
                avatar_url: None,
                canonical_alias: None,
                guest_can_join: false,
                join_rule: "private".to_string(), // Default, should be fetched from room state
                name: None,
                num_joined_members: 0,
                room_type: Some("m.space".to_string()),
                topic: None,
                world_readable: false,
                children_state: surreal_hierarchy.room.children_state.into_iter().map(|event| SpaceChildEvent {
                    content: SpaceChildContent {
                        via: vec![], // Should be populated from event content
                        order: None,
                        suggested: None,
                    },
                    origin_server_ts: chrono::Utc::now().timestamp_millis() as u64,
                    sender: event.sender,
                    state_key: event.state_key,
                    event_type: event.event_type,
                }).collect(),
            };
            rooms.push(root_room);
            
            // Convert child rooms
            for child in surreal_hierarchy.children {
                let child_room = SpaceHierarchyRoom {
                    room_id: child.room_id,
                    avatar_url: None,
                    canonical_alias: None,
                    guest_can_join: false,
                    join_rule: "private".to_string(), // Should be fetched from room state
                    name: None,
                    num_joined_members: 0,
                    room_type: child.room_type,
                    topic: None,
                    world_readable: false,
                    children_state: vec![], // Child rooms don't include their own children in this response
                };
                rooms.push(child_room);
            }

            let hierarchy_response = HierarchyResponse {
                rooms,
                next_batch: None, // Pagination not implemented yet
            };

            Ok(Json(hierarchy_response))
        },
        Err(e) => {
            error!("Failed to get room hierarchy for room {}: {}", room_id, e);
            match e {
                matryx_surrealdb::repository::error::RepositoryError::NotFound { .. } => {
                    Err(StatusCode::NOT_FOUND)
                },
                matryx_surrealdb::repository::error::RepositoryError::Unauthorized { .. } => {
                    Err(StatusCode::FORBIDDEN)
                },
                matryx_surrealdb::repository::error::RepositoryError::Validation { .. } => {
                    Err(StatusCode::BAD_REQUEST)
                },
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
    }
}
