use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::{
    EventRepository,
    MembershipRepository,
    RoomRepository,
    UserRepository,
    error::RepositoryError,
    room_join::RoomJoinService,
};

#[derive(Deserialize)]
pub struct JoinRequest {
    /// Optional reason for joining
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Optional third-party signed token for invite validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_signed: Option<Value>,
}

#[derive(Serialize)]
pub struct JoinResponse {
    pub room_id: String,
}

/// Matrix Client-Server API v1.11 Section 10.2.1
///
/// POST /_matrix/client/v3/join/{roomIdOrAlias}
///
/// Join a room by room ID or room alias. This endpoint allows authenticated
/// users to join public rooms or rooms they have been invited to.
///
/// For public rooms, the user can join directly. For invite-only rooms,
/// the user must have a pending invitation.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id_or_alias): Path<String>,
    Json(request): Json<JoinRequest>,
) -> Result<Json<JoinResponse>, StatusCode> {
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

    // Handle optional join reason for audit logging
    if let Some(reason) = &request.reason {
        info!(
            "Room join with reason - User: {}, Room: {}, Reason: '{}', From: {}",
            user_id, room_id_or_alias, reason, addr
        );
    } else {
        info!(
            "Processing room join request for user: {} to room: {} from: {}",
            user_id, room_id_or_alias, addr
        );
    }

    // Handle third-party signed invitations if provided
    if let Some(_third_party_signed) = &request.third_party_signed {
        info!("Third-party signed invitation detected for user: {}", user_id);
        // TODO: Implement proper third-party signed invitation validation
        // This involves verifying cryptographic signatures from identity servers
        // For now, we log the presence but proceed with standard join
    }

    // Create repository instances
    let room_repo = RoomRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());
    let event_repo = EventRepository::new(state.db.clone());
    let user_repo = UserRepository::new(state.db.clone());

    // Create room join service
    let join_service = RoomJoinService::new(room_repo, membership_repo, event_repo, user_repo);

    // Use the join service to handle the room join
    match join_service.join_room(&room_id_or_alias, &user_id).await {
        Ok(result) => {
            info!(
                "Successfully joined user {} to room {} with event {}",
                user_id, result.room_id, result.event_id
            );
            Ok(Json(JoinResponse { room_id: result.room_id }))
        },
        Err(e) => {
            match e {
                RepositoryError::NotFound { .. } => {
                    warn!("Room join failed - room not found: {}", room_id_or_alias);
                    Err(StatusCode::NOT_FOUND)
                },
                RepositoryError::Unauthorized { .. } => {
                    warn!(
                        "Room join failed - user {} not authorized to join room {}",
                        user_id, room_id_or_alias
                    );
                    Err(StatusCode::FORBIDDEN)
                },
                RepositoryError::Validation { .. } => {
                    warn!(
                        "Room join failed - invalid room identifier format: {}",
                        room_id_or_alias
                    );
                    Err(StatusCode::BAD_REQUEST)
                },
                _ => {
                    error!("Room join failed - internal error: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                },
            }
        },
    }
}
