use axum::{Json, extract::{Path, Query, State}, http::{StatusCode, HeaderMap}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::{
    auth::{authenticate_user, MatrixAuthError},
    state::AppState,
};

#[derive(Deserialize)]
pub struct ContextParams {
    pub limit: Option<u32>,
    pub filter: Option<String>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/context/{eventId}
pub async fn get(
    Path((room_id, event_id)): Path<(String, String)>,
    Query(params): Query<ContextParams>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    info!("Event context requested for event {} in room {}", event_id, room_id);

    // 1. Validate user access to room
    let user_id = authenticate_user(&state, &headers).await
        .map_err(|e| match e {
            MatrixAuthError::MissingToken | MatrixAuthError::InvalidToken => StatusCode::UNAUTHORIZED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    if let Err(_) = validate_room_access(&state, &room_id, &user_id).await {
        warn!("User {} does not have access to room {}", user_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // 2. Get target event
    let target_event = match get_event(&state, &room_id, &event_id).await {
        Ok(event) => event,
        Err(_) => {
            warn!("Event {} not found in room {}", event_id, room_id);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // 3. Get events before and after
    let limit = params.limit.unwrap_or(10).min(100); // Cap at 100 events
    let events_before = get_events_before(&state, &room_id, target_event.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0), limit).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let events_after = get_events_after(&state, &room_id, target_event.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0), limit).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 4. Get room state at time of event
    let state_events = get_room_state_at_time(&state, &room_id, target_event.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0)).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 5. Generate pagination tokens
    let start_token = format!("t{}_start", target_event.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0));
    let end_token = format!("t{}_end", target_event.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0));

    info!("Successfully retrieved context for event {} in room {}", event_id, room_id);

    Ok(Json(json!({
        "start": start_token,
        "end": end_token,
        "events_before": events_before,
        "event": target_event,
        "events_after": events_after,
        "state": state_events
    })))
}

async fn validate_room_access(state: &AppState, room_id: &str, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
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
        .await?;

    let membership_events: Vec<Value> = result.take(0)?;

    if let Some(event) = membership_events.first() {
        if let Some(membership) = event.get("membership").and_then(|v| v.as_str()) {
            if membership == "join" {
                return Ok(());
            }
        }
    }

    // Check if room allows history visibility for non-members
    let history_visibility = get_room_history_visibility(state, room_id).await?;
    if matches!(history_visibility.as_str(), "world_readable" | "shared") {
        return Ok(());
    }

    Err("User does not have access to room".into())
}

async fn get_event(state: &AppState, room_id: &str, event_id: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let query = "
        SELECT type, content, sender, origin_server_ts, event_id, room_id, state_key
        FROM room_timeline_events 
        WHERE room_id = $room_id AND event_id = $event_id
        LIMIT 1
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("event_id", event_id))
        .await?;

    let events: Vec<Value> = result.take(0)?;
    
    events.into_iter().next()
        .ok_or_else(|| "Event not found".into())
}

async fn get_events_before(state: &AppState, room_id: &str, timestamp: u64, limit: u32) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let query = "
        SELECT type, content, sender, origin_server_ts, event_id, room_id, state_key
        FROM room_timeline_events 
        WHERE room_id = $room_id AND origin_server_ts < $timestamp
        ORDER BY origin_server_ts DESC
        LIMIT $limit
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("timestamp", timestamp))
        .bind(("limit", limit))
        .await?;

    let mut events: Vec<Value> = result.take(0)?;
    events.reverse(); // Return in chronological order
    Ok(events)
}

async fn get_events_after(state: &AppState, room_id: &str, timestamp: u64, limit: u32) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let query = "
        SELECT type, content, sender, origin_server_ts, event_id, room_id, state_key
        FROM room_timeline_events 
        WHERE room_id = $room_id AND origin_server_ts > $timestamp
        ORDER BY origin_server_ts ASC
        LIMIT $limit
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("timestamp", timestamp))
        .bind(("limit", limit))
        .await?;

    let events: Vec<Value> = result.take(0)?;
    Ok(events)
}

async fn get_room_state_at_time(state: &AppState, room_id: &str, timestamp: u64) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let query = "
        SELECT DISTINCT ON (type, state_key) type, content, sender, origin_server_ts, event_id, room_id, state_key
        FROM room_state_events 
        WHERE room_id = $room_id AND origin_server_ts <= $timestamp
        ORDER BY type, state_key, origin_server_ts DESC
        LIMIT 50
    ";
    
    let mut result = state.db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("timestamp", timestamp))
        .await?;

    let events: Vec<Value> = result.take(0)?;
    Ok(events)
}

async fn get_room_history_visibility(state: &AppState, room_id: &str) -> Result<String, Box<dyn std::error::Error>> {
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
        .await?;

    let visibility_events: Vec<Value> = result.take(0)?;

    if let Some(event) = visibility_events.first() {
        if let Some(visibility) = event.get("history_visibility").and_then(|v| v.as_str()) {
            return Ok(visibility.to_string());
        }
    }

    // Default to "shared" if no history visibility event found
    Ok("shared".to_string())
}