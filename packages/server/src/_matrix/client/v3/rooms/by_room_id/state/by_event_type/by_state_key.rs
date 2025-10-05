use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, warn};

use crate::auth::AuthenticatedUser;
use crate::federation::event_signer::EventSigner;
use crate::state::AppState;
use matryx_entity::types::Event;
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

/// GET /_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}
///
/// Get the current state event of the given type and state key for the room.
pub async fn get(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((room_id, event_type, state_key)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    debug!(
        "Getting state event {}:{} for room: {} by user: {}",
        event_type, state_key, room_id, auth.user_id
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

    // Get current state event of the specified type and state_key
    let state_event = get_current_state_event(&state, &room_id, &event_type, &state_key)
        .await
        .map_err(|e| {
            error!("Failed to get state event: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            debug!("State event {}:{} not found in room {}", event_type, state_key, room_id);
            StatusCode::NOT_FOUND
        })?;

    debug!("Retrieved state event {}:{} for room {}", event_type, state_key, room_id);
    Ok(Json(state_event))
}

/// PUT /_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}
///
/// Send a state event to the room. The state event will replace any existing
/// state event of the same type and state key.
pub async fn put(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((room_id, event_type, state_key)): Path<(String, String, String)>,
    Json(content): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    debug!(
        "Setting state event {}:{} for room: {} by user: {}",
        event_type, state_key, room_id, auth.user_id
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

    // Check if user has permission to send state events
    let has_permission = check_state_send_permission(&state, &room_id, &auth.user_id, &event_type)
        .await
        .map_err(|e| {
            error!("Failed to check state send permissions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !has_permission {
        warn!("User {} not authorized to send state events to room {}", auth.user_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate state event content based on type
    if let Err(validation_error) = validate_state_event_content(&event_type, &content) {
        warn!("Invalid state event content: {}", validation_error);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create and send the state event
    let event_id =
        send_state_event(&state, &room_id, &auth.user_id, &event_type, &state_key, content)
            .await
            .map_err(|e| {
                error!("Failed to send state event: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    debug!(
        "Successfully sent state event {}:{} for room {} with event_id: {}",
        event_type, state_key, room_id, event_id
    );

    Ok(Json(json!({
        "event_id": event_id
    })))
}

/// Check if a user has permission to view room state using repository
async fn check_room_state_permission(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Check user's membership in the room
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let membership = membership_repo
        .get_user_membership_status(room_id, user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // User can view state if they are joined, invited, or have left (for historical state)
    if let Some(membership) = membership {
        return Ok(matches!(membership.as_str(), "join" | "invite" | "leave"));
    }

    // Check if room has world-readable history
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let world_readable = room_repo
        .is_room_world_readable(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(world_readable)
}

/// Check if user has permission to send state events using repository
async fn check_state_send_permission(
    state: &AppState,
    room_id: &str,
    user_id: &str,
    event_type: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // User must be joined to send state events
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let membership = membership_repo
        .get_user_membership_status(room_id, user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    if !matches!(membership, Some(ref m) if m == "join") {
        return Ok(false);
    }

    // Get user's power level and required power level for this event type
    let power_levels = get_room_power_levels(state, room_id).await?;
    let user_power = get_user_power_level(&power_levels, user_id)?;
    let required_power = get_required_power_level(&power_levels, event_type)?;

    Ok(user_power >= required_power)
}

/// Get current state event by type and state_key using repository
async fn get_current_state_event(
    state: &AppState,
    room_id: &str,
    event_type: &str,
    state_key: &str,
) -> Result<Option<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let event = event_repo
        .get_current_state_event(room_id, event_type, state_key)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

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

/// Get room power levels using repository
async fn get_room_power_levels(
    state: &AppState,
    room_id: &str,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let power_levels = event_repo
        .get_room_power_levels(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(power_levels)
}

/// Get user's power level from power_levels content
fn get_user_power_level(
    power_levels: &Value,
    user_id: &str,
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    // Check explicit user power levels
    if let Some(users) = power_levels.get("users").and_then(|u| u.as_object())
        && let Some(user_power) = users.get(user_id).and_then(|p| p.as_i64())
    {
        return Ok(user_power);
    }

    // Default user power level
    Ok(power_levels.get("users_default").and_then(|d| d.as_i64()).unwrap_or(0))
}

/// Get required power level for an event type
fn get_required_power_level(
    power_levels: &Value,
    event_type: &str,
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    // Check specific event type power levels
    if let Some(events) = power_levels.get("events").and_then(|e| e.as_object())
        && let Some(required_power) = events.get(event_type).and_then(|p| p.as_i64())
    {
        return Ok(required_power);
    }

    // Default state event power level
    Ok(power_levels.get("state_default").and_then(|d| d.as_i64()).unwrap_or(50))
}

/// Validate state event content based on event type
fn validate_state_event_content(event_type: &str, content: &Value) -> Result<(), String> {
    match event_type {
        "m.room.name" => {
            if let Some(name) = content.get("name")
                && !name.is_string()
            {
                return Err("name must be a string".to_string());
            }
        },
        "m.room.topic" => {
            if let Some(topic) = content.get("topic")
                && !topic.is_string()
            {
                return Err("topic must be a string".to_string());
            }
        },
        "m.room.avatar" => {
            if let Some(url) = content.get("url")
                && !url.is_string()
            {
                return Err("url must be a string".to_string());
            }
        },
        "m.room.canonical_alias" => {
            if let Some(alias) = content.get("alias")
                && !alias.is_string()
                && !alias.is_null()
            {
                return Err("alias must be a string or null".to_string());
            }
        },
        _ => {
            // Allow other event types without validation
        },
    }

    Ok(())
}

/// Send a state event to the room
async fn send_state_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    event_type: &str,
    state_key: &str,
    content: Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Generate event ID
    let event_id = format!("${}", uuid::Uuid::new_v4());
    let origin_server_ts = chrono::Utc::now().timestamp_millis();

    // Get forward extremities for prev_events (DAG compliance)
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let prev_events = event_repo.get_prev_events(room_id).await.map_err(|e| {
        error!("Failed to get prev_events for room {}: {}", room_id, e);
        Box::new(std::io::Error::other("Failed to get prev_events")) as Box<dyn std::error::Error + Send + Sync>
    })?;

    // Calculate depth per Matrix spec: max(prev_events.depth) + 1
    let depth = event_repo.calculate_event_depth(&prev_events).await.map_err(|e| {
        error!("Failed to calculate event depth: {}", e);
        Box::new(std::io::Error::other("Failed to calculate depth")) as Box<dyn std::error::Error + Send + Sync>
    })?;

    // Get auth events for authorization validation (DAG compliance)
    let auth_events = event_repo.get_auth_events(room_id, event_type, sender, state_key).await.map_err(|e| {
        error!("Failed to get auth_events for room {}: {}", room_id, e);
        Box::new(std::io::Error::other("Failed to get auth_events")) as Box<dyn std::error::Error + Send + Sync>
    })?;

    // Create the event using the entity constructor
    let mut event = Event::new(
        event_id.clone(),
        sender.to_string(),
        origin_server_ts,
        event_type.to_string(),
        room_id.to_string(),
        matryx_entity::EventContent::unknown(content),
    );

    // Set additional fields for state events
    event.state_key = Some(state_key.to_string());
    event.depth = Some(depth);
    event.received_ts = Some(chrono::Utc::now().timestamp_millis());
    event.outlier = Some(false);

    // Populate with proper auth events (for Matrix DAG compliance)
    event.auth_events = Some(auth_events);

    // Populate with prev events (for Matrix DAG compliance)
    event.prev_events = Some(prev_events);

    // Sign the event using proper Matrix cryptographic signatures
    // This replaces the dangerous Default::default() stubs
    let event_signer = match EventSigner::from_app_state(
        state.session_service.clone(),
        state.db.clone(),
        state.dns_resolver.clone(),
        state.homeserver_name.clone(),
    ) {
        Ok(signer) => signer,
        Err(e) => {
            error!("Failed to create event signer: {:?}", e);
            return Err(Box::new(std::io::Error::other("Failed to create event signer")));
        },
    };

    // Sign the event with proper Matrix signatures and hashes
    let mut signed_event = event.clone();
    if let Err(e) = event_signer.sign_outgoing_event(&mut signed_event, None).await {
        error!("Failed to sign state event: {:?}", e);
        return Err(Box::new(std::io::Error::other("Failed to sign state event")));
    }

    // Store the properly signed event
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    event_repo.create(&signed_event).await?;

    Ok(event_id)
}
