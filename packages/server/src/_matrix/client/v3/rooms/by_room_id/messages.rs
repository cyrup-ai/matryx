use axum::{Json, extract::{Path, Query, State}, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{AppState, auth::{MatrixAuth, extract_matrix_auth}};
use matryx_entity::types::RoomEventFilter;

#[derive(Debug, Deserialize)]
pub struct MessagesQueryParams {
    /// The token to start returning events from (pagination token)
    pub from: Option<String>,
    /// The token to stop returning events at (pagination token)
    pub to: Option<String>,
    /// The direction to return events from: 'b' = backwards (default), 'f' = forwards
    #[serde(default = "default_direction")]
    pub dir: String,
    /// The maximum number of events to return (default 10)
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// A JSON RoomEventFilter to filter returned events
    pub filter: Option<String>,
}

fn default_direction() -> String {
    "b".to_string()
}

fn default_limit() -> u32 {
    10
}

#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    /// The token the pagination starts from
    pub start: String,
    /// The token the pagination ends at
    pub end: String,
    /// A list of room events
    pub chunk: Vec<Value>,
    /// A list of state events (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Vec<Value>>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/messages
///
/// Get a list of message and state events for a room with pagination.
/// This allows clients to paginate through the timeline of a room.
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Query(params): Query<MessagesQueryParams>,
) -> Result<Json<MessagesResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room messages request failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room messages request failed - access token expired");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room messages request failed - server authentication not typically used");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room messages request failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room messages request for room: {} user: {} dir: {} limit: {}",
        room_id, user_id, params.dir, params.limit
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate direction parameter
    if params.dir != "b" && params.dir != "f" {
        warn!("Invalid direction parameter: {}", params.dir);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse filter parameter if provided
    let filter = if let Some(filter_str) = &params.filter {
        match serde_json::from_str::<RoomEventFilter>(filter_str) {
            Ok(f) => Some(f),
            Err(e) => {
                warn!("Invalid filter JSON: {}", e);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    } else {
        None
    };

    // TODO: Validate user has access to room
    // For now, we'll proceed with the query

    // Get paginated messages from database
    match state.room_operations.room_repo().get_room_messages_paginated(
        &room_id,
        params.from.as_deref(),
        params.to.as_deref(),
        &params.dir,
        params.limit,
        filter.as_ref(),
    ).await {
        Ok((events, start_token, end_token)) => {
            // Convert events to JSON
            let chunk: Vec<Value> = events
                .into_iter()
                .map(|event| serde_json::to_value(event).unwrap_or(json!({})))
                .collect();

            info!(
                "Returning {} events for room {} (start: {}, end: {})",
                chunk.len(),
                room_id,
                start_token,
                end_token
            );

            Ok(Json(MessagesResponse {
                start: start_token,
                end: end_token,
                chunk,
                state: None, // State events can be added in future enhancement
            }))
        },
        Err(e) => {
            error!("Failed to get room messages: {}", e);
            match e {
                matryx_surrealdb::repository::error::RepositoryError::NotFound { .. } => {
                    Err(StatusCode::NOT_FOUND)
                },
                matryx_surrealdb::repository::error::RepositoryError::Validation { .. } => {
                    Err(StatusCode::BAD_REQUEST)
                },
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
    }
}
