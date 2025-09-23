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

#[derive(Deserialize)]
pub struct InviteRequest {
    /// The Matrix user ID of the user to invite
    pub user_id: String,

    /// Optional reason for the invitation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct InviteResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.2
///
/// POST /_matrix/client/v3/rooms/{roomId}/invite
///
/// Invite a user to join a room. The authenticated user must have permission
/// to invite users to the room (based on power levels and room configuration).
///
/// The invited user will receive an invitation event and can choose to accept
/// or reject the invitation using the join or leave endpoints respectively.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<InviteRequest>,
) -> Result<Json<InviteResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room invite failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let inviter_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room invite failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room invite failed - server authentication not allowed for room invites");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room invite failed - anonymous authentication not allowed for room invites");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room invitation from user: {} to invite: {} to room: {} (from: {})",
        inviter_id, request.user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room invite failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate user ID format
    if !request.user_id.starts_with('@') || !request.user_id.contains(':') {
        warn!("Room invite failed - invalid user ID format: {}", request.user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to invite user with all validation
    match state
        .room_operations
        .invite_user(&room_id, &request.user_id, &inviter_id, request.reason)
        .await
    {
        Ok(()) => {
            info!(
                "Successfully invited user {} to room {} by {}",
                request.user_id, room_id, inviter_id
            );
            Ok(Json(InviteResponse {}))
        },
        Err(e) => {
            error!("Failed to invite user {} to room {}: {}", request.user_id, room_id, e);
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
