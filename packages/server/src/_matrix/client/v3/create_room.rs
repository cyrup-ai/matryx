use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};


use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    utils::matrix_identifiers::generate_room_id,
};
use matryx_surrealdb::repository::{
    EventRepository,
    MembershipRepository,
    PowerLevelsRepository,
    RoomManagementService,
    RoomRepository,
    error::RepositoryError,
    room::RoomCreationConfig,
};

#[derive(Deserialize)]
pub struct CreateRoomRequest {
    visibility: Option<String>, // "public" or "private"
    room_alias_name: Option<String>,
    name: Option<String>,
    topic: Option<String>,
    invite: Option<Vec<String>>,
    invite_3pid: Option<Vec<Value>>,
    room_version: Option<String>,
    creation_content: Option<Value>,
    initial_state: Option<Vec<InitialStateEvent>>,
    preset: Option<String>, // "private_chat", "public_chat", "trusted_private_chat"
    is_direct: Option<bool>,
    power_level_content_override: Option<Value>,
}

#[derive(Deserialize)]
struct InitialStateEvent {
    #[serde(rename = "type")]
    event_type: String,
    state_key: Option<String>,
    content: Value,
}

#[derive(Serialize)]
pub struct CreateRoomResponse {
    room_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_alias: Option<String>,
}

/// Matrix Client-Server API v1.11 Section 10.1.1
///
/// POST /_matrix/client/v3/createRoom
///
/// Create a new room with the given configuration. This endpoint supports all
/// Matrix room creation features including state events, invitations, power levels,
/// and room presets for different types of rooms.
///
/// This endpoint requires authentication and will create the room on behalf of
/// the authenticated user who becomes the room creator and initial member.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room creation failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room creation failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room creation failed - server authentication not allowed for room creation");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room creation failed - anonymous authentication not allowed for room creation");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room creation request for user: {} from: {}", user_id, addr);

    // Validate room version if provided
    if let Some(ref version) = request.room_version
        && !is_supported_room_version(version) {
        warn!("Room creation failed - unsupported room version: {}", version);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Generate room ID for the new room
    let room_id = generate_room_id();
    info!("Generated room ID: {} for user: {}", room_id, user_id);

    // Create repository instances
    let room_repo = RoomRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());
    let event_repo = EventRepository::new(state.db.clone());
    let power_levels_repo = PowerLevelsRepository::new(state.db.clone());

    // Create room management service
    let room_service =
        RoomManagementService::new(room_repo, event_repo, membership_repo, power_levels_repo);

    // Determine visibility
    let visibility = request.visibility.clone().unwrap_or_else(|| "private".to_string());
    let is_public = visibility == "public";

    // Create room configuration
    let room_config = RoomCreationConfig {
        name: request.name.clone(),
        topic: request.topic.clone(),
        alias: request.room_alias_name.clone(),
        is_public,
        is_direct: request.is_direct.unwrap_or(false),
        preset: request.preset.clone(),
        invite_users: request.invite.clone().unwrap_or_default(),
        initial_state: request
            .initial_state
            .as_ref()
            .map(|states| {
                states
                    .iter()
                    .map(|state| {
                        serde_json::json!({
                            "type": state.event_type,
                            "state_key": state.state_key,
                            "content": state.content
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        power_level_content_override: request.power_level_content_override.clone(),
        invite_3pid: request.invite_3pid.clone().unwrap_or_default(),
        creation_content: request.creation_content.clone(),
    };

    // Use the room management service to create the room
    match room_service.create_room(&user_id, room_config).await {
        Ok(room) => {
            let elapsed = start_time.elapsed();
            info!(
                "Successfully created room {} for user {} in {:?}",
                room.room_id, user_id, elapsed
            );

            // Handle third party invites if provided
            if let Some(ref invite_3pid) = request.invite_3pid {
                for invite in invite_3pid {
                    if let (Some(medium), Some(address), Some(id_server)) =
                        (invite.get("medium").and_then(|v| v.as_str()),
                         invite.get("address").and_then(|v| v.as_str()),
                         invite.get("id_server").and_then(|v| v.as_str())) {
                        info!("Processing third party invite: {} {} via {}", medium, address, id_server);
                        // TODO: Implement third party invite processing
                        // This would involve contacting the identity server to validate the invite
                    }
                }
            }

            // Apply creation content if provided (custom room creation parameters)
            if let Some(ref creation_content) = request.creation_content {
                info!("Applying custom creation content for room {}: {:?}", room.room_id, creation_content);
                // TODO: Store custom creation content in room metadata
                // This content is typically used for room versioning or custom room types
            }

            // Create room alias if requested
            let room_alias = if let Some(alias_name) = request.room_alias_name {
                let full_alias = format!("#{}:{}", alias_name, state.homeserver_name);
                // TODO: Create alias in database
                Some(full_alias)
            } else {
                None
            };

            Ok(Json(CreateRoomResponse { room_id: room.room_id, room_alias }))
        },
        Err(e) => {
            match e {
                RepositoryError::Validation { .. } => {
                    warn!("Room creation failed - validation error: {}", e);
                    Err(StatusCode::BAD_REQUEST)
                },
                RepositoryError::Unauthorized { .. } => {
                    warn!("Room creation failed - unauthorized: {}", e);
                    Err(StatusCode::FORBIDDEN)
                },
                _ => {
                    error!("Room creation failed - internal error: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        },
    }
}



fn is_supported_room_version(version: &str) -> bool {
    matches!(version, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11")
}
