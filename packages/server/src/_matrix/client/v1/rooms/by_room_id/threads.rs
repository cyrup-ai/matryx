use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::types::ThreadSummary;
use matryx_surrealdb::repository::threads::ThreadInclude;

#[derive(Deserialize)]
pub struct ThreadsQuery {
    pub include: Option<String>, // "all" or "participated"
    pub from: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct ThreadsResponse {
    pub chunk: Vec<ThreadSummary>,
    pub next_token: Option<String>,
    pub prev_token: Option<String>,
}

/// GET /_matrix/client/v1/rooms/{roomId}/threads
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Query(query): Query<ThreadsQuery>,
) -> Result<Json<ThreadsResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room threads request failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room threads request failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            // Server-to-server requests are allowed for federation
            "server".to_string()
        },
        MatrixAuth::Anonymous => {
            warn!("Room threads request failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room threads request for room: {} by user: {}", room_id, user_id);

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room threads request failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Convert include string to ThreadInclude enum
    let include = query.include.as_deref().map(|s| {
        match s {
            "all" => ThreadInclude::All,
            "participated" => ThreadInclude::Participated,
            _ => ThreadInclude::All, // Default to All for unknown values
        }
    });

    // Apply limit parameter from query for pagination
    let limit = query.limit.unwrap_or(10).min(100); // Default 10, max 100 per Matrix spec
    if limit != 10 {
        info!("Room threads limit override requested: {}", limit);
    }

    // Use RoomOperationsService to get thread roots with all validation
    match state
        .room_operations
        .get_thread_roots(&room_id, &user_id, include, query.from.as_deref())
        .await
    {
        Ok(thread_roots_response) => {
            info!(
                "Successfully retrieved {} threads for room {}",
                thread_roots_response.threads.len(),
                room_id
            );

            // Convert ThreadRootsResponse to ThreadsResponse (Matrix spec compliant)
            let mut chunk: Vec<matryx_entity::types::ThreadSummary> = Vec::new();

            for thread_root in thread_roots_response.threads {
                // Convert from repository ThreadSummary to entity ThreadSummary per Matrix spec
                let repo_summary = thread_root.unsigned.thread;
                let thread_root_id = thread_root.event_id.clone();

                // Fetch participants for this thread
                let participants_list = state
                    .room_operations
                    .threads_repo()
                    .get_thread_participants(&room_id, &thread_root_id)
                    .await
                    .unwrap_or_else(|_| Vec::new());

                let participants: Vec<String> = participants_list
                    .into_iter()
                    .filter(|p| p.participating)
                    .map(|p| p.user_id)
                    .collect();

                chunk.push(matryx_entity::types::ThreadSummary {
                    latest_event: Some(repo_summary.latest_event),
                    count: repo_summary.count as usize,
                    participated: repo_summary.current_user_participated,
                    participants,
                    notification_count: None,
                    highlight_count: None,
                });
            }

            let threads_response = ThreadsResponse {
                chunk,
                next_token: thread_roots_response.next_batch,
                prev_token: thread_roots_response.prev_batch,
            };

            Ok(Json(threads_response))
        },
        Err(e) => {
            error!("Failed to get threads for room {}: {}", room_id, e);
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
