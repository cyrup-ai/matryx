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
use matryx_entity::types::Event;
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

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
    let room = room_repo
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
    let has_permission = check_room_state_permission(&state, &room_id, &auth.user_id)
        .await
        .map_err(|e| {
            error!("Failed to check room state permissions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !has_permission {
        warn!("User {} not authorized to view state of room {}", auth.user_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Get current state event of the specified type (with empty state_key by default)
    let state_event = get_current_state_event(&state, &room_id, &event_type, "")
        .await
        .map_err(|e| {
            error!("Failed to get state event: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            debug!("State event type {} not found in room {}", event_type, room_id);
            StatusCode::NOT_FOUND
        })?;

    debug!("Retrieved state event {} for room {}", event_type, room_id);
    Ok(Json(state_event))
}

/// Check if a user has permission to view room state
async fn check_room_state_permission(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Check user's membership in the room
    let query = "
        SELECT membership
        FROM membership
        WHERE room_id = $room_id AND user_id = $user_id
        ORDER BY updated_at DESC
        LIMIT 1
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("user_id", user_id.to_string()))
        .await?;

    #[derive(serde::Deserialize)]
    struct MembershipResult {
        membership: String,
    }

    let membership: Option<MembershipResult> = response.take(0)?;

    // User can view state if they are joined, invited, or have left (for historical state)
    if let Some(membership) = membership {
        return Ok(matches!(membership.membership.as_str(), "join" | "invite" | "leave"));
    }

    // Check if room has world-readable history
    let world_readable = is_room_world_readable(state, room_id).await?;
    Ok(world_readable)
}

/// Check if a room has world-readable history
async fn is_room_world_readable(
    state: &AppState,
    room_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT content.history_visibility
        FROM event
        WHERE room_id = $room_id
        AND type = 'm.room.history_visibility'
        AND state_key = ''
        ORDER BY depth DESC, origin_server_ts DESC
        LIMIT 1
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct HistoryVisibility {
        history_visibility: Option<String>,
    }

    let visibility: Option<HistoryVisibility> = response.take(0)?;
    let history_visibility = visibility
        .and_then(|v| v.history_visibility)
        .unwrap_or_else(|| "shared".to_string());

    Ok(history_visibility == "world_readable")
}

/// Get current state event by type and state_key
async fn get_current_state_event(
    state: &AppState,
    room_id: &str,
    event_type: &str,
    state_key: &str,
) -> Result<Option<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT *
        FROM event
        WHERE room_id = $room_id
        AND type = $event_type
        AND state_key = $state_key
        ORDER BY depth DESC, origin_server_ts DESC
        LIMIT 1
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("event_type", event_type.to_string()))
        .bind(("state_key", state_key.to_string()))
        .await?;

    let event: Option<Event> = response.take(0)?;

    if let Some(event) = event {
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
        Ok(Some(state_event))
    } else {
        Ok(None)
    }
}
