use axum::{Json, extract::{Path, Query, State}, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{AppState, auth::{MatrixAuth, extract_matrix_auth}};
use matryx_entity::types::RoomEventFilter;
use matryx_surrealdb::repository::RoomRepository;

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

/// Validates pagination token format without parsing
/// Format: t{timestamp}_{event_id}
fn is_valid_pagination_token(token: &str) -> bool {
    if !token.starts_with('t') {
        return false;
    }
    
    let parts: Vec<&str> = token[1..].splitn(2, '_').collect();
    if parts.len() != 2 {
        return false;
    }
    
    // Check timestamp is numeric
    if parts[0].parse::<i64>().is_err() {
        return false;
    }
    
    // Check event_id starts with $
    if !parts[1].starts_with('$') {
        return false;
    }
    
    true
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

    let (user_id, is_guest) = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room messages request failed - access token expired");
                return Err(StatusCode::UNAUTHORIZED);
            }
            
            // Get session to check if user is a guest
            let session_repo = matryx_surrealdb::repository::SessionRepository::new(state.db.clone());
            let session = session_repo
                .get_by_access_token(&token_info.token)
                .await
                .map_err(|e| {
                    error!("Failed to get session: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            
            let is_guest = session.map(|s| s.is_guest).unwrap_or(false);
            (token_info.user_id.clone(), is_guest)
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

    // Check guest access before membership
    let room_repo = RoomRepository::new(state.db.clone());
    crate::room::authorization::require_room_access(&room_repo, &room_id, &user_id, is_guest)
        .await?;

    // Validate user has access to room
    let is_member = state
        .room_operations
        .membership_repo()
        .is_user_in_room(&room_id, &user_id)
        .await
        .map_err(|e| {
            error!("Failed to check room membership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !is_member {
        warn!(
            "User {} attempted to access messages in room {} without membership",
            user_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Pre-validate pagination tokens for faster rejection and better error messages
    if let Some(from_str) = params.from.as_ref() {
        if !is_valid_pagination_token(from_str) {
            warn!("Invalid 'from' pagination token format: {}", from_str);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    if let Some(to_str) = params.to.as_ref() {
        if !is_valid_pagination_token(to_str) {
            warn!("Invalid 'to' pagination token format: {}", to_str);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Validate and enforce limit bounds
    let limit = params.limit;
    if limit == 0 {
        warn!("Invalid limit parameter: 0");
        return Err(StatusCode::BAD_REQUEST);
    }
    if limit > 100 {
        warn!("Limit {} exceeds maximum allowed (100)", limit);
        return Err(StatusCode::BAD_REQUEST);
    }

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
