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
pub struct BanRequest {
    /// The Matrix user ID of the user to ban
    pub user_id: String,

    /// Optional reason for the ban
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct BanResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.5
///
/// POST /_matrix/client/v3/rooms/{roomId}/ban
///
/// Ban a user from a room. The authenticated user must have sufficient power
/// level to ban users in the room. The banned user will be immediately removed
/// from the room and will not be able to join until they are unbanned.
///
/// This endpoint can be used to ban users who are currently in the room or
/// users who are not currently in the room (preemptive ban).
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<BanRequest>,
) -> Result<Json<BanResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room ban failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let banner_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room ban failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room ban failed - server authentication not allowed for room bans");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room ban failed - anonymous authentication not allowed for room bans");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room ban request from user: {} to ban: {} from room: {} (from: {})",
        banner_id, request.user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room ban failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate user ID format
    if !request.user_id.starts_with('@') || !request.user_id.contains(':') {
        warn!("Room ban failed - invalid user ID format: {}", request.user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Cannot ban yourself
    if banner_id == request.user_id {
        warn!("Room ban failed - user {} cannot ban themselves", banner_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to ban user with all validation
    match state
        .room_operations
        .ban_user(&room_id, &request.user_id, &banner_id, request.reason)
        .await
    {
        Ok(()) => {
            info!(
                "Successfully banned user {} from room {} by {}",
                request.user_id, room_id, banner_id
            );
            Ok(Json(BanResponse {}))
        },
        Err(e) => {
            error!("Failed to ban user {} from room {}: {}", request.user_id, room_id, e);
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
