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
pub struct ReportEventRequest {
    pub reason: String,
    pub score: Option<i32>, // -100 to 0 (most offensive)
}

#[derive(Serialize)]
pub struct ReportResponse {
    // Empty response per Matrix spec
}

/// POST /_matrix/client/v3/rooms/{roomId}/report/{eventId}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((room_id, event_id)): Path<(String, String)>,
    Json(request): Json<ReportEventRequest>,
) -> Result<Json<ReportResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Event report failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Event report failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Event report failed - server authentication not allowed for event reports");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Event report failed - anonymous authentication not allowed for event reports");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing event report for event {} in room {} by user {}", event_id, room_id, user_id);

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Event report failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate event ID format
    if !event_id.starts_with('$') {
        warn!("Event report failed - invalid event ID format: {}", event_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to report event with all validation
    match state
        .room_operations
        .report_event(&room_id, &event_id, &user_id, &request.reason, request.score)
        .await
    {
        Ok(()) => {
            info!(
                "Successfully reported event {} in room {} by user {}",
                event_id, room_id, user_id
            );
            Ok(Json(ReportResponse {}))
        },
        Err(e) => {
            error!("Failed to report event {} in room {}: {}", event_id, room_id, e);
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
