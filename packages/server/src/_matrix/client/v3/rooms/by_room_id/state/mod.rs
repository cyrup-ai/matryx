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

    // Get current room state
    let state_events = get_current_room_state(&state, &room_id).await.map_err(|e| {
        error!("Failed to get room state: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    debug!("Retrieved {} state events for room {}", state_events.len(), room_id);
    Ok(Json(json!(state_events)))
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

/// Get current room state events
async fn get_current_room_state(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Get all current state events for the room
    let query = "
        SELECT *
        FROM event
        WHERE room_id = $room_id
        AND state_key IS NOT NULL
        AND (
            SELECT COUNT()
            FROM event e2
            WHERE e2.room_id = $room_id
            AND e2.type = event.type
            AND e2.state_key = event.state_key
            AND (e2.depth > event.depth OR (e2.depth = event.depth AND e2.origin_server_ts > event.origin_server_ts))
        ) = 0
        ORDER BY type, state_key
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let events: Vec<Event> = response.take(0)?;

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

    Ok(state_events)
}

pub mod by_event_type;
