use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Serialize)]
pub struct RoomAliasesResponse {
    aliases: Vec<String>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/aliases
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
) -> Result<Json<RoomAliasesResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room aliases request failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room aliases request failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            // Server-to-server requests are allowed for federation
            "server".to_string()
        },
        MatrixAuth::Anonymous => {
            warn!("Room aliases request failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room aliases request for room: {} by user: {}", room_id, user_id);

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room aliases request failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to get room aliases with all validation
    match state.room_operations.get_room_aliases(&room_id, &user_id).await {
        Ok(aliases_response) => {
            info!(
                "Successfully retrieved {} aliases for room {}",
                aliases_response.aliases.len(),
                room_id
            );
            Ok(Json(RoomAliasesResponse { aliases: aliases_response.aliases }))
        },
        Err(e) => {
            error!("Failed to get room aliases for room {}: {}", room_id, e);
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
