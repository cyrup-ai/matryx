use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};


use serde::{Deserialize, Serialize};


use tracing::{error, info, warn};


use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::types::MembershipState;
use matryx_surrealdb::repository::{MembershipRepository, RoomRepository};

#[derive(Deserialize)]
pub struct KickRequest {
    /// The Matrix user ID of the user to kick
    pub user_id: String,

    /// Optional reason for the kick
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct KickResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.6
///
/// POST /_matrix/client/v3/rooms/{roomId}/kick
///
/// Kick (remove) a user from a room. The authenticated user must have sufficient
/// power level to kick users in the room. The kicked user will be removed from
/// the room but will be able to rejoin if they have appropriate permissions.
///
/// This is different from banning - kicked users can rejoin, banned users cannot.
/// The target user must currently be joined to the room to be kicked.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<KickRequest>,
) -> Result<Json<KickResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room kick failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let kicker_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room kick failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room kick failed - server authentication not allowed for room kicks");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room kick failed - anonymous authentication not allowed for room kicks");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room kick request from user: {} to kick: {} from room: {} (from: {})",
        kicker_id, request.user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room kick failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate user ID format
    if !request.user_id.starts_with('@') || !request.user_id.contains(':') {
        warn!("Room kick failed - invalid user ID format: {}", request.user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Cannot kick yourself
    if kicker_id == request.user_id {
        warn!("Room kick failed - user {} cannot kick themselves", kicker_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomRepository to check if room exists
    let room_repo = RoomRepository::new(state.db.clone());
    let _room = room_repo.get_by_id(&room_id).await.map_err(|e| {
        error!("Failed to query room {}: {}", room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.ok_or_else(|| {
        warn!("Room kick failed - room not found: {}", room_id);
        StatusCode::NOT_FOUND
    })?;

    // Use MembershipRepository to check memberships and permissions
    let membership_repo = MembershipRepository::new(state.db.clone());
    
    // Check if kicker is a member of the room with appropriate permissions
    let kicker_membership = membership_repo.get_membership(&room_id, &kicker_id).await.map_err(|_| {
        warn!("Room kick failed - kicker {} is not a member of room {}", kicker_id, room_id);
        StatusCode::FORBIDDEN
    })?;

    if let Some(membership) = kicker_membership {
        if membership.membership != MembershipState::Join {
            warn!(
                "Room kick failed - kicker {} must be joined to room {} to kick users",
                kicker_id, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }
    } else {
        warn!("Room kick failed - kicker {} is not a member of room {}", kicker_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Check target user's current membership
    let target_membership = membership_repo.get_membership(&room_id, &request.user_id).await.map_err(|_| {
        warn!(
            "Room kick failed - target user {} is not a member of room {}",
            request.user_id, room_id
        );
        StatusCode::FORBIDDEN
    })?;

    // Target must be currently joined to be kicked
    if let Some(membership) = target_membership {
        match membership.membership {
            MembershipState::Join => {
                // Good, can proceed with kick
            },
            MembershipState::Leave => {
                info!("User {} has already left room {}", request.user_id, room_id);
                return Ok(Json(KickResponse {}));
            },
            MembershipState::Ban => {
                warn!("User {} is already banned from room {}", request.user_id, room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            _ => {
                warn!(
                    "Cannot kick user {} with membership state {:?} in room {}",
                    request.user_id, membership.membership, room_id
                );
                return Err(StatusCode::FORBIDDEN);
            },
        }
    } else {
        warn!(
            "Room kick failed - target user {} is not a member of room {}",
            request.user_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Use membership repository to check if user can perform kick action
    let can_kick = membership_repo.can_perform_action(
        &room_id, 
        &kicker_id, 
        matryx_surrealdb::repository::room_operations::MembershipAction::Kick, 
        Some(&request.user_id)
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_kick {
        warn!(
            "Room kick failed - user {} does not have permission to kick {} in room {}",
            kicker_id, request.user_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Use membership repository to perform the kick
    membership_repo.kick_member(&room_id, &request.user_id, &kicker_id, request.reason.as_deref())
        .await
        .map_err(|e| {
            error!(
                "Failed to kick user {} from room {} by {}: {}",
                request.user_id, room_id, kicker_id, e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let kick_event_id = format!("$kick_{}_{}", request.user_id, chrono::Utc::now().timestamp_millis());

    info!(
        "Successfully kicked user {} from room {} by {} with event {}",
        request.user_id, room_id, kicker_id, kick_event_id
    );

    Ok(Json(KickResponse {}))
}
