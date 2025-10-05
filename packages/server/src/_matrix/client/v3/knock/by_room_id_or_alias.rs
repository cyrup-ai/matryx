use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;

use serde::{Deserialize, Serialize};

use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::types::{Membership, MembershipState};
use matryx_surrealdb::repository::{
    event::EventRepository, membership::MembershipRepository, room::RoomRepository,
    user::UserRepository,
};

#[derive(Deserialize)]
pub struct KnockRequest {
    /// Optional reason for knocking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct KnockResponse {
    pub room_id: String,
}

/// Matrix Client-Server API MSC2403 - Knocking
///
/// POST /_matrix/client/v3/knock/{roomIdOrAlias}
///
/// Request to join a room that has knock permissions enabled. This sends a
/// knock membership event that room moderators can see and respond to by
/// inviting the user or ignoring the request.
///
/// Only works for rooms with join_rule set to "knock". Users cannot knock
/// on rooms they are already members of, invited to, or banned from.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id_or_alias): Path<String>,
    Json(request): Json<KnockRequest>,
) -> Result<Json<KnockResponse>, StatusCode> {
    // Initialize repositories
    let room_repo = RoomRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());
    let event_repo = EventRepository::new(state.db.clone());
    let user_repo = UserRepository::new(state.db.clone());

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room knock failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room knock failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room knock failed - server authentication not allowed for room knocks");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room knock failed - anonymous authentication not allowed for room knocks");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room knock request for user: {} to room: {} from: {}",
        user_id, room_id_or_alias, addr
    );

    // Resolve room ID from alias if necessary
    let actual_room_id = if room_id_or_alias.starts_with('#') {
        // Room alias - need to resolve to room ID
        room_repo
            .resolve_room_alias(&room_id_or_alias)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or_else(|| {
                warn!("Room knock failed - could not resolve room alias: {}", room_id_or_alias);
                StatusCode::NOT_FOUND
            })?
    } else if room_id_or_alias.starts_with('!') {
        // Already a room ID
        room_id_or_alias.clone()
    } else {
        warn!("Room knock failed - invalid room identifier format: {}", room_id_or_alias);
        return Err(StatusCode::BAD_REQUEST);
    };

    // Check if user is already in the room
    if let Ok(Some(current_membership)) =
        membership_repo.get_by_room_user(&actual_room_id, &user_id).await
    {
        match current_membership.membership {
            MembershipState::Join => {
                warn!("User {} is already joined to room {}", user_id, actual_room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            MembershipState::Invite => {
                warn!("User {} is already invited to room {}", user_id, actual_room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            MembershipState::Ban => {
                warn!(
                    "Room knock failed - user {} is banned from room {}",
                    user_id, actual_room_id
                );
                return Err(StatusCode::FORBIDDEN);
            },
            MembershipState::Knock => {
                info!("User {} has already knocked on room {}", user_id, actual_room_id);
                return Ok(Json(KnockResponse { room_id: actual_room_id }));
            },
            MembershipState::Leave => {
                // User previously left - can knock
            },
        }
    }

    // Get room information to check knock permissions
    let room = room_repo
        .get_by_id(&actual_room_id)
        .await
        .map_err(|e| {
            error!("Failed to query room {}: {}", actual_room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Room knock failed - room not found: {}", actual_room_id);
            StatusCode::NOT_FOUND
        })?;

    // Check if room allows knocking
    match room.join_rules.as_deref() {
        Some("knock") => {
            // Room allows knocking - proceed
        },
        Some("public") => {
            warn!("Room knock failed - room {} is public, use join instead", actual_room_id);
            return Err(StatusCode::BAD_REQUEST);
        },
        Some("invite") => {
            warn!("Room knock failed - room {} is invite-only", actual_room_id);
            return Err(StatusCode::FORBIDDEN);
        },
        Some("restricted") | Some("private") => {
            warn!(
                "Room knock failed - room {} does not allow knocking (restricted/private)",
                actual_room_id
            );
            return Err(StatusCode::FORBIDDEN);
        },
        _ => {
            warn!(
                "Room knock failed - room {} does not allow knocking (unknown join rule)",
                actual_room_id
            );
            return Err(StatusCode::FORBIDDEN);
        },
    }

    // Get event depth and prev/auth events using repositories
    let prev_events = event_repo
        .get_prev_events(&actual_room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _event_depth = event_repo
        .calculate_event_depth(&prev_events)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _auth_events = event_repo
        .get_auth_events_for_knock(&actual_room_id, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create membership event using repository
    let knock_event_id = event_repo
        .create_membership_event(&actual_room_id, &user_id, MembershipState::Knock)
        .await
        .map_err(|e| {
            error!(
                "Failed to create knock event for user {} in room {}: {}",
                user_id, actual_room_id, e
            );
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
        .is_direct_message_room(&actual_room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create/update membership record using repository
    let membership_record = Membership {
        user_id: user_id.clone(),
        room_id: actual_room_id.clone(),
        membership: MembershipState::Knock,
        reason: request.reason.clone(),
        invited_by: None, // Not applicable for knock events
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
        "Successfully created knock event {} for user {} in room {}",
        knock_event_id.event_id, user_id, actual_room_id
    );

    Ok(Json(KnockResponse { room_id: actual_room_id }))
}
