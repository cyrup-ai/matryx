use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use matryx_surrealdb::repository::RoomRepository;
use serde_json::{Value, json};
use tracing::{error, info, warn};

/// GET /_matrix/client/v3/rooms/{roomId}/initialSync
///
/// This endpoint supports room previews for world_readable rooms.
/// Users can preview room content without joining if history_visibility is set to "world_readable".
pub async fn get(
    Path(room_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    info!("Room initial sync requested for room: {}", room_id);

    // Initialize room repository
    let room_repo = RoomRepository::new(state.db.clone());

    // Validate authentication using session service
    // This endpoint supports optional authentication for world_readable room previews
    let user_id: Option<String> = if let Some(auth_header) = headers.get("authorization")
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
        && let Ok(access_token) = state.session_service.validate_access_token(token).await
        && !access_token.is_expired()
    {
        Some(access_token.user_id)
    } else {
        None
    };

    // Check room history visibility
    let history_visibility =
        room_repo.get_room_history_visibility(&room_id).await.map_err(|e| {
            error!("Failed to get room history visibility: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if history_visibility != "world_readable" && user_id.is_none() {
        warn!("Room {} is not world_readable and user is not authenticated", room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    if history_visibility != "world_readable"
        && let Some(ref authenticated_user) = user_id
    {
        // Check if user is member of the room
        let is_member = room_repo
            .check_room_membership(&room_id, authenticated_user)
            .await
            .map_err(|e| {
                error!("Failed to check room membership: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if !is_member {
            warn!("User {} is not a member of room {}", authenticated_user, room_id);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Get room state events
    let state_events = room_repo.get_room_state_events(&room_id).await.map_err(|e| {
        error!("Failed to get room state events: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get recent messages (limited for preview)
    let messages = room_repo.get_room_messages(&room_id, 20).await.map_err(|e| {
        error!("Failed to get room messages: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get room presence (empty for preview)
    let presence: Vec<Value> = vec![];

    // Get account data (empty for preview)
    let account_data: Vec<Value> = vec![];

    info!(
        "Successfully retrieved initial sync for room {} (preview mode: {})",
        room_id,
        user_id.is_none()
    );

    Ok(Json(json!({
        "room_id": room_id,
        "messages": {
            "start": "preview_start",
            "end": "preview_end",
            "chunk": messages
        },
        "state": state_events,
        "presence": presence,
        "account_data": account_data
    })))
}

// All helper functions have been moved to RoomRepository
