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

#[derive(Deserialize)]
pub struct RelationsQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<u32>,
    pub dir: Option<String>, // "f" or "b"
}


#[derive(Serialize, Deserialize)]
pub struct ReactionAggregation {
    pub key: String,
    pub count: u64,
    pub users: Vec<String>,
}

#[derive(Serialize)]
pub struct RelationsResponse {
    pub chunk: Vec<Event>,
    pub aggregations: HashMap<String, ReactionAggregation>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
}

/// GET /_matrix/client/v1/rooms/{roomId}/relations/{eventId}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((room_id, event_id)): Path<(String, String)>,
    Query(query): Query<RelationsQuery>,
) -> Result<Json<RelationsResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Event relations request failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Event relations request failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            // Server-to-server requests are allowed for federation
            "server".to_string()
        },
        MatrixAuth::Anonymous => {
            warn!("Event relations request failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing event relations request for event {} in room {} by user {}",
        event_id, room_id, user_id
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Event relations request failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate event ID format
    if !event_id.starts_with('$') {
        warn!("Event relations request failed - invalid event ID format: {}", event_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Use RoomOperationsService to get event relations with all validation
    match state.room_operations.get_event_relations(
        &room_id,
        &event_id,
        &user_id,
        None, // rel_type (all relation types)
        None, // event_type
        query.limit,
        query.from.as_deref(),
        query.to.as_deref(),
        query.dir.as_deref(),
    ).await {
        Ok(relations_response) => {
            info!("Successfully retrieved event relations for event {} in room {}", event_id, room_id);
            Ok(Json(relations_response))
        },
        Err(e) => {
            error!("Failed to get event relations for event {} in room {}: {}", event_id, room_id, e);
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
        }
    }
}