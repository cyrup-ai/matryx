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
pub struct LeaveRequest {
    /// Optional reason for leaving the room
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct LeaveResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.3
///
/// POST /_matrix/client/v3/rooms/{roomId}/leave
///
/// Leave a room that the user is currently joined to. This endpoint creates
/// a leave membership event for the authenticated user in the specified room.
///
/// The user must currently be a member of the room (have "join" membership)
/// to be able to leave it. After leaving, the user will no longer receive
/// events from the room unless they rejoin.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<LeaveRequest>,
) -> Result<Json<LeaveResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room leave failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room leave failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room leave failed - server authentication not allowed for room leaves");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room leave failed - anonymous authentication not allowed for room leaves");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room leave request for user: {} from room: {} (from: {})",
        user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room leave failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to leave room with all validation
    match state.room_operations.leave_room(&room_id, &user_id, request.reason).await {
        Ok(()) => {
            info!("Successfully left room {} for user {}", room_id, user_id);
            Ok(Json(LeaveResponse {}))
        },
        Err(e) => {
            error!("Failed to leave room {} for user {}: {}", room_id, user_id, e);
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
