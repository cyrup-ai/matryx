use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    auth::{MatrixAuthError, extract_matrix_auth},
    state::AppState,
};

#[derive(Deserialize)]
pub struct ContextParams {
    pub limit: Option<u32>,
    pub filter: Option<String>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/context/{eventId}
pub async fn get(
    Path((room_id, event_id)): Path<(String, String)>,
    Query(params): Query<ContextParams>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    info!("Event context requested for event {} in room {}", event_id, room_id);

    // Extract user authentication
    let matrix_auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        match e {
            MatrixAuthError::MissingToken | MatrixAuthError::MissingAuthorization => {
                StatusCode::UNAUTHORIZED
            },
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;

    let user_id = match matrix_auth {
        crate::auth::MatrixAuth::User(user_auth) => user_auth.user_id,
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    // Use RoomOperationsService to get event context with permission validation
    let limit = params.limit.unwrap_or(10).min(100); // Cap at 100 events

    match state
        .room_operations
        .get_event_context(&room_id, &event_id, limit, &user_id)
        .await
    {
        Ok(context_response) => {
            info!("Successfully retrieved context for event {} in room {}", event_id, room_id);

            // Convert ContextResponse to Matrix API format
            Ok(Json(json!({
                "start": context_response.start,
                "end": context_response.end,
                "events_before": context_response.events_before,
                "event": context_response.event,
                "events_after": context_response.events_after,
                "state": context_response.state
            })))
        },
        Err(e) => {
            error!("Failed to get event context for event {} in room {}: {}", event_id, room_id, e);
            match e {
                matryx_surrealdb::repository::error::RepositoryError::NotFound { .. } => {
                    Err(StatusCode::NOT_FOUND)
                },
                matryx_surrealdb::repository::error::RepositoryError::Unauthorized { .. } => {
                    Err(StatusCode::FORBIDDEN)
                },
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
    }
}
