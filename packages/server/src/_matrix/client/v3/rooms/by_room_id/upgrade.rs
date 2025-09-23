use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Deserialize)]
pub struct RoomUpgradeRequest {
    pub new_version: String,
}

#[derive(Serialize)]
pub struct RoomUpgradeResponse {
    pub replacement_room: String,
}

/// POST /_matrix/client/v3/rooms/{roomId}/upgrade
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<RoomUpgradeRequest>,
) -> Result<Json<RoomUpgradeResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room upgrade failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room upgrade failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room upgrade failed - server authentication not allowed for room upgrades");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room upgrade failed - anonymous authentication not allowed for room upgrades");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room upgrade request for room {} to version {} by user {}",
        room_id, request.new_version, user_id
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room upgrade failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to upgrade room with all validation
    match state
        .room_operations
        .upgrade_room(&room_id, &user_id, &request.new_version)
        .await
    {
        Ok(upgrade_response) => {
            info!(
                "Successfully upgraded room {} to {} (new room: {:?})",
                room_id, request.new_version, upgrade_response
            );
            Ok(Json(RoomUpgradeResponse {
                replacement_room: upgrade_response.replacement_room,
            }))
        },
        Err(e) => {
            error!("Failed to upgrade room {}: {}", room_id, e);
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
