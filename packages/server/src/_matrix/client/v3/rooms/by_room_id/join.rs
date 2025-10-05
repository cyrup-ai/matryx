use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::types::{Membership, MembershipState, Room};
use matryx_surrealdb::repository::{
    event::EventRepository, membership::MembershipRepository, room::RoomRepository,
    user::UserRepository,
};

#[derive(Deserialize)]
pub struct JoinRequest {
    /// Optional reason for joining
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Optional third-party signed token for invite validation
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)] // Matrix protocol field - will be implemented for third-party invites
    pub third_party_signed: Option<Value>,
}

#[derive(Serialize)]
pub struct JoinResponse {
    pub room_id: String,
}

/// Matrix Client-Server API v1.11 Section 10.2.4
///
/// POST /_matrix/client/v3/rooms/{roomId}/join
///
/// Join a room by room ID. This endpoint allows authenticated users to join
/// public rooms or rooms they have been invited to. Unlike the general join
/// endpoint, this one only accepts room IDs, not room aliases.
///
/// For public rooms, the user can join directly. For invite-only rooms,
/// the user must have a pending invitation.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<JoinRequest>,
) -> Result<Json<JoinResponse>, StatusCode> {
    // Initialize repositories
    let room_repo = RoomRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());
    let event_repo = EventRepository::new(state.db.clone());
    let user_repo = UserRepository::new(state.db.clone());

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room join failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room join failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room join failed - server authentication not allowed for room joins");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room join failed - anonymous authentication not allowed for room joins");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room join request for user: {} to room: {} from: {}", user_id, room_id, addr);

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room join failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if user is already in the room
    if let Ok(Some(current_membership)) = membership_repo.get_by_room_user(&room_id, &user_id).await
    {
        match current_membership.membership {
            MembershipState::Join => {
                info!("User {} already joined room {}", user_id, room_id);
                return Ok(Json(JoinResponse { room_id }));
            },
            MembershipState::Ban => {
                warn!("Room join failed - user {} is banned from room {}", user_id, room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            _ => {
                // User has some other membership state (invite, leave, knock) - proceed with join
            },
        }
    }

    // Get room information to check join rules
    let room = room_repo
        .get_by_id(&room_id)
        .await
        .map_err(|e| {
            error!("Failed to query room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Room join failed - room not found: {}", room_id);
            StatusCode::NOT_FOUND
        })?;

    // Check join authorization based on room join rules
    if !can_user_join_via_repositories(&room_repo, &membership_repo, &room, &user_id).await? {
        warn!("Room join failed - user {} not authorized to join room {}", user_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Get event depth and prev/auth events using repositories
    let prev_events = event_repo
        .get_prev_events(&room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _event_depth = event_repo
        .calculate_event_depth(&prev_events)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _auth_events = event_repo
        .get_auth_events_for_join(&room_id, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create membership event using repository
    let join_event_id = event_repo
        .create_membership_event(&room_id, &user_id, MembershipState::Join)
        .await
        .map_err(|e| {
            error!("Failed to create join event for user {} in room {}: {}", user_id, room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get user profile information from repository
    let display_name = user_repo
        .get_user_display_name(&user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let avatar_url = user_repo
        .get_user_avatar_url(&user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_direct = room_repo
        .is_direct_message_room(&room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create/update membership record using repository
    let membership_record = Membership {
        user_id: user_id.clone(),
        room_id: room_id.clone(),
        membership: MembershipState::Join,
        reason: request.reason.clone(),
        invited_by: None,
        updated_at: Some(Utc::now()),
        display_name,
        avatar_url,
        is_direct: Some(is_direct),
        third_party_invite: None,
        join_authorised_via_users_server: None,
    };

    membership_repo
        .upsert_membership(&membership_record)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(
        "Successfully created join event {} for user {} in room {}",
        join_event_id.event_id, user_id, room_id
    );

    Ok(Json(JoinResponse { room_id }))
}

async fn can_user_join_via_repositories(
    room_repo: &RoomRepository,
    membership_repo: &MembershipRepository,
    room: &Room,
    user_id: &str,
) -> Result<bool, StatusCode> {
    match room.join_rules.as_deref() {
        Some("public") => Ok(true),
        Some("invite") => {
            // Check if user has pending invitation
            match membership_repo.get_by_room_user(&room.room_id, user_id).await {
                Ok(Some(membership)) => Ok(membership.membership == MembershipState::Invite),
                _ => Ok(false),
            }
        },
        Some("knock") => {
            // Check if user has sent a knock request
            match membership_repo.get_by_room_user(&room.room_id, user_id).await {
                Ok(Some(membership)) => Ok(membership.membership == MembershipState::Knock),
                _ => Ok(false),
            }
        },
        Some("restricted") => {
            // MSC3083: Restricted room access rules
            // First check for pending invite
            if let Ok(Some(membership)) =
                membership_repo.get_by_room_user(&room.room_id, user_id).await
                && membership.membership == MembershipState::Invite
            {
                return Ok(true);
            }

            // Check allow conditions from room's join_rules state event
            let allow_conditions = room_repo
                .get_join_rule_allow_conditions(&room.room_id)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to get join rule allow conditions for room {}: {}",
                        room.room_id, e
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Check if user is a member of any of the allowed rooms/spaces
            for condition in allow_conditions {
                if condition.get("type").and_then(|v| v.as_str()) == Some("m.room_membership")
                    && let Some(allowed_room_id) = condition.get("room_id").and_then(|v| v.as_str())
                    && let Ok(Some(membership)) =
                        membership_repo.get_by_room_user(allowed_room_id, user_id).await
                    && membership.membership == MembershipState::Join
                {
                    info!(
                        "User {} allowed to join restricted room {} via membership in room {}",
                        user_id, room.room_id, allowed_room_id
                    );
                    return Ok(true);
                }
            }
            Ok(false)
        },
        Some("private") => Ok(false), // Private join rule
        _ => Ok(false),               // Unknown join rule
    }
}
