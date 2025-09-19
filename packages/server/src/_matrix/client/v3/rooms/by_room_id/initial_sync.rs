use axum::{Json, extract::{Path, State}, http::{StatusCode, HeaderMap}};
use serde_json::{Value, json};
use crate::state::AppState;
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

    // Check if user is authenticated (optional for previews)
    let user_id = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .and_then(|token| {
            // In a real implementation, validate token and get user_id
            // For now, return None to simulate unauthenticated preview
            None
        });

    // Check room history visibility
    let history_visibility = get_room_history_visibility(&state, &room_id).await?;
    
    if history_visibility != "world_readable" && user_id.is_none() {
        warn!("Room {} is not world_readable and user is not authenticated", room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    if history_visibility != "world_readable" && user_id.is_some() {
        // Check if user is member of the room
        let is_member = check_room_membership(&state, &room_id, user_id.as_ref().unwrap()).await?;
        if !is_member {
            warn!("User {} is not a member of room {}", user_id.unwrap(), room_id);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Get room state events
    let state_events = get_room_state_events(&state, &room_id).await?;
    
    // Get recent messages (limited for preview)
    let messages = get_room_messages(&state, &room_id, 20).await?;
    
    // Get room presence (empty for preview)
    let presence: Vec<Value> = vec![];
    
    // Get account data (empty for preview)
    let account_data: Vec<Value> = vec![];

    info!("Successfully retrieved initial sync for room {} (preview mode: {})", 
          room_id, user_id.is_none());

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

async fn get_room_history_visibility(state: &AppState, room_id: &str) -> Result<String, StatusCode> {
    let query = "
        SELECT content.history_visibility
        FROM room_state_events 
        WHERE room_id = $room_id AND type = 'm.room.history_visibility' AND state_key = ''
        ORDER BY origin_server_ts DESC
        LIMIT 1
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .await
        .map_err(|e| {
            error!("Database error getting history visibility: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let visibility_events: Vec<Value> = result
        .take(0)
        .map_err(|e| {
            error!("Error parsing history visibility result: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Some(event) = visibility_events.first() {
        if let Some(visibility) = event.get("history_visibility").and_then(|v| v.as_str()) {
            return Ok(visibility.to_string());
        }
    }

    // Default to "shared" if no history visibility event found
    Ok("shared".to_string())
}

async fn check_room_membership(state: &AppState, room_id: &str, user_id: &str) -> Result<bool, StatusCode> {
    let query = "
        SELECT content.membership
        FROM room_memberships 
        WHERE room_id = $room_id AND user_id = $user_id
        ORDER BY origin_server_ts DESC
        LIMIT 1
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("user_id", user_id))
        .await
        .map_err(|e| {
            error!("Database error checking membership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let membership_events: Vec<Value> = result
        .take(0)
        .map_err(|e| {
            error!("Error parsing membership result: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Some(event) = membership_events.first() {
        if let Some(membership) = event.get("membership").and_then(|v| v.as_str()) {
            return Ok(membership == "join");
        }
    }

    Ok(false)
}

async fn get_room_state_events(state: &AppState, room_id: &str) -> Result<Vec<Value>, StatusCode> {
    let query = "
        SELECT type, state_key, content, sender, origin_server_ts, event_id
        FROM room_state_events 
        WHERE room_id = $room_id
        ORDER BY origin_server_ts DESC
        LIMIT 50
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .await
        .map_err(|e| {
            error!("Database error getting state events: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let state_events: Vec<Value> = result
        .take(0)
        .map_err(|e| {
            error!("Error parsing state events result: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(state_events)
}

async fn get_room_messages(state: &AppState, room_id: &str, limit: u32) -> Result<Vec<Value>, StatusCode> {
    let query = "
        SELECT type, content, sender, origin_server_ts, event_id
        FROM room_timeline_events 
        WHERE room_id = $room_id AND type = 'm.room.message'
        ORDER BY origin_server_ts DESC
        LIMIT $limit
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("limit", limit))
        .await
        .map_err(|e| {
            error!("Database error getting messages: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let messages: Vec<Value> = result
        .take(0)
        .map_err(|e| {
            error!("Error parsing messages result: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(messages)
}