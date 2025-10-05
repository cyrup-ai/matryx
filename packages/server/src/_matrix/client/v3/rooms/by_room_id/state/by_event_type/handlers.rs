use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, warn};

use crate::auth::AuthenticatedUser;
use crate::state::AppState;

use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

/// GET /_matrix/client/v3/rooms/{roomId}/state/{eventType}
///
/// Get the current state event of the given type for the room.
/// If there are multiple state events with the same type, returns the one with empty state_key.
pub async fn get(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((room_id, event_type)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    debug!(
        "Getting state event type: {} for room: {} by user: {}",
        event_type, room_id, auth.user_id
    );

    // Validate room exists
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let _room = room_repo
        .get_by_id(&room_id)
        .await
        .map_err(|e| {
            error!("Failed to query room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Room {} not found", room_id);
            StatusCode::NOT_FOUND
        })?;

    // Check if user has permission to view room state
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let membership = membership_repo
        .get_user_membership_status(&room_id, &auth.user_id)
        .await
        .map_err(|e| {
            error!("Failed to check user membership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let has_permission = if let Some(membership) = membership {
        matches!(membership.as_str(), "join" | "invite" | "leave")
    } else {
        // Check if room has world-readable history
        room_repo.is_room_world_readable(&room_id).await.map_err(|e| {
            error!("Failed to check room world-readable status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    if !has_permission {
        warn!("User {} not authorized to view state of room {}", auth.user_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current state event of the specified type (with empty state_key by default)
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let event = event_repo
        .get_current_state_event(&room_id, &event_type, "")
        .await
        .map_err(|e| {
            error!("Failed to get state event: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            debug!("State event type {} not found in room {}", event_type, room_id);
            StatusCode::NOT_FOUND
        })?;

    let state_event = json!({
        "event_id": event.event_id,
        "type": event.event_type,
        "room_id": event.room_id,
        "sender": event.sender,
        "content": event.content,
        "state_key": event.state_key,
        "origin_server_ts": event.origin_server_ts,
        "unsigned": event.unsigned.unwrap_or_default()
    });

    debug!("Retrieved state event {} for room {}", event_type, room_id);
    Ok(Json(state_event))
}
