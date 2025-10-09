use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};
use uuid::Uuid;
use chrono::Utc;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    utils::matrix_identifiers::generate_room_id,
};
use matryx_entity::types::{SignedThirdPartyInvite, Event, EventContent};
use matryx_surrealdb::repository::{
    EventRepository, MembershipRepository, PowerLevelsRepository, RoomAliasRepository, RoomManagementService,
    RoomRepository, error::RepositoryError, room::RoomCreationConfig,
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

/// Validate third-party invite with identity server
async fn validate_third_party_invite(
    medium: &str,
    address: &str,
    id_server: &str,
    id_access_token: Option<&str>,
) -> Result<SignedThirdPartyInvite, RepositoryError> {
    use reqwest::Client;

    // Build identity server URL
    let url = format!("https://{}/_matrix/identity/v2/store-invite", id_server);

    // Create HTTP client
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| RepositoryError::ExternalService(e.to_string()))?;

    // Prepare request body
    let body = serde_json::json!({
        "medium": medium,
        "address": address,
    });

    // Make request with optional access token
    let mut request = client.post(&url).json(&body);
    if let Some(token) = id_access_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    // Send request and parse response
    let response = request
        .send()
        .await
        .map_err(|e| RepositoryError::ExternalService(e.to_string()))?;

    if !response.status().is_success() {
        return Err(RepositoryError::ExternalService(format!(
            "Identity server returned {}",
            response.status()
        )));
    }

    let signed_invite: SignedThirdPartyInvite = response
        .json()
        .await
        .map_err(|e| RepositoryError::SerializationError {
            message: e.to_string(),
        })?;

    Ok(signed_invite)
}

/// Create third-party invite state event
async fn create_third_party_invite_event(
    event_repo: &EventRepository,
    room_id: &str,
    sender: &str,
    display_name: &str,
    signed: &SignedThirdPartyInvite,
    id_server: &str,
) -> Result<Event, RepositoryError> {
    let event_id = format!("${}", Uuid::new_v4());
    let now = Utc::now();

    let content = serde_json::json!({
        "display_name": display_name,
        "key_validity_url": format!("https://{}/_matrix/identity/api/v1/pubkey/isvalid", id_server),
        "public_key": signed.signatures.values().next()
            .and_then(|sigs| sigs.keys().next())
            .map(|k| k.as_str())
            .unwrap_or(""),
        "public_keys": signed.signatures.iter()
            .flat_map(|(_, sigs)| sigs.keys())
            .map(|k| k.as_str())
            .collect::<Vec<_>>(),
    });

    let event = Event {
        event_id: event_id.clone(),
        room_id: room_id.to_string(),
        sender: sender.to_string(),
        event_type: "m.room.third_party_invite".to_string(),
        content: EventContent::Unknown(content),
        state_key: Some(signed.token.clone()),
        origin_server_ts: now.timestamp_millis(),
        unsigned: None,
        prev_events: None,
        auth_events: None,
        depth: None,
        hashes: None,
        signatures: None,
        redacts: None,
        outlier: Some(false),
        received_ts: Some(now.timestamp_millis()),
        rejected_reason: None,
        soft_failed: Some(false),
    };

    event_repo.create(&event).await
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
        && !is_supported_room_version(version)
    {
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
                let event_repo = EventRepository::new(state.db.clone());

                for invite in invite_3pid {
                    if let (Some(medium), Some(address), Some(id_server)) = (
                        invite.get("medium").and_then(|v| v.as_str()),
                        invite.get("address").and_then(|v| v.as_str()),
                        invite.get("id_server").and_then(|v| v.as_str()),
                    ) {
                        info!(
                            "Processing third party invite: {} {} via {}",
                            medium, address, id_server
                        );

                        // Get optional ID access token
                        let id_access_token = invite.get("id_access_token").and_then(|v| v.as_str());

                        // Validate with identity server
                        match validate_third_party_invite(medium, address, id_server, id_access_token).await {
                            Ok(signed_invite) => {
                                // Create display name from address
                                let display_name = invite
                                    .get("display_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(address);

                                // Create third party invite state event
                                if let Err(e) = create_third_party_invite_event(
                                    &event_repo,
                                    &room.room_id,
                                    &user_id,
                                    display_name,
                                    &signed_invite,
                                    id_server,
                                )
                                .await
                                {
                                    error!("Failed to create third party invite event: {}", e);
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Failed to validate third party invite with {}: {}",
                                    id_server, e
                                );
                                // Continue processing other invites
                            }
                        }
                    }
                }
            }

            // Create room alias if requested
            let room_alias = if let Some(alias_name) = request.room_alias_name {
                let full_alias = format!("#{}:{}", alias_name, state.homeserver_name);

                // Create alias in database
                let alias_repo = RoomAliasRepository::new(state.db.clone());

                match alias_repo.create_alias(&full_alias, &room.room_id, &user_id).await {
                    Ok(_) => {
                        // Set as canonical alias (optional but recommended)
                        if let Err(e) = alias_repo.set_canonical_alias(
                            &room.room_id,
                            Some(&full_alias),
                            &user_id
                        ).await {
                            warn!("Failed to set canonical alias for room {}: {}", room.room_id, e);
                            // Continue anyway - alias was created
                        }
                        Some(full_alias)
                    }
                    Err(e) => {
                        warn!("Failed to create room alias {}: {}", full_alias, e);
                        // Don't fail room creation if alias creation fails
                        None
                    }
                }
            } else {
                None
            };

            Ok(Json(CreateRoomResponse { room_id: room.room_id, room_alias }))
        },
        Err(e) => match e {
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
        },
    }
}

fn is_supported_room_version(version: &str) -> bool {
    matches!(version, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11")
}
