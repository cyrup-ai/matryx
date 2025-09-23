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
pub struct ForgetRequest {
    // The forget request has no body parameters per Matrix specification
}

#[derive(Serialize)]
pub struct ForgetResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.8
///
/// POST /_matrix/client/v3/rooms/{roomId}/forget
///
/// Forget a room that the user has previously left. This removes the room from
/// the user's room list and clears their local state for the room. The user
/// must have previously left the room (have "leave" membership state) to be
/// able to forget it.
///
/// This is primarily a client-side operation that removes the user's membership
/// record but does not create any events in the room itself.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(_request): Json<ForgetRequest>,
) -> Result<Json<ForgetResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room forget failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room forget failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room forget failed - server authentication not allowed for room forget");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room forget failed - anonymous authentication not allowed for room forget");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room forget request for user: {} to forget room: {} (from: {})",
        user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room forget failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to forget room with all validation
    match state.room_operations.forget_room(&room_id, &user_id).await {
        Ok(()) => {
            info!("Successfully forgot room {} for user {}", room_id, user_id);
            Ok(Json(ForgetResponse {}))
        },
        Err(e) => {
            error!("Failed to forget room {} for user {}: {}", room_id, user_id, e);
            match e {
                matryx_surrealdb::repository::error::RepositoryError::NotFound { .. } => {
                    Err(StatusCode::BAD_REQUEST)
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
