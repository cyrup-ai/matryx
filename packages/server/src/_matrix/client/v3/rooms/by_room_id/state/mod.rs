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

/// GET /_matrix/client/v3/rooms/{roomId}/state
///
/// Get the current state of the room. Returns all state events for the room.
pub async fn get(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(room_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    debug!("Getting room state for room: {} by user: {}", room_id, auth.user_id);

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
    let membership_status = membership_repo.get_user_membership_status(&room_id, &auth.user_id)
        .await
        .map_err(|e| {
            error!("Failed to check user membership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let has_permission = if let Some(membership) = membership_status {
        matches!(membership.as_str(), "join" | "invite" | "leave")
    } else {
        // Check if room has world-readable history
        room_repo.is_room_world_readable(&room_id)
            .await
            .map_err(|e| {
                error!("Failed to check room world-readable status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    };

    if !has_permission {
        warn!("User {} not authorized to view state of room {}", auth.user_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current room state
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let events = event_repo.get_room_current_state(&room_id, None)
        .await
        .map_err(|e| {
            error!("Failed to get room state: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Convert events to Matrix client format
    let state_events: Vec<Value> = events
        .into_iter()
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "type": event.event_type,
                "room_id": event.room_id,
                "sender": event.sender,
                "content": event.content,
                "state_key": event.state_key,
                "origin_server_ts": event.origin_server_ts,
                "unsigned": event.unsigned.unwrap_or_default()
            })
        })
        .collect();

    debug!("Retrieved {} state events for room {}", state_events.len(), room_id);
    Ok(Json(json!(state_events)))
}



pub mod by_event_type;
