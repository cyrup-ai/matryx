use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::Room;
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

/// Query parameters for state request
#[derive(Debug, Deserialize)]
pub struct StateQuery {
    /// An event ID in the room to retrieve the state at
    event_id: String,
}

/// Matrix X-Matrix authentication header parsed structure
#[derive(Debug, Clone)]
struct XMatrixAuth {
    origin: String,
    key_id: String,
    signature: String,
}

/// Parse X-Matrix authentication header
fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<XMatrixAuth, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if !auth_header.starts_with("X-Matrix ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_params = &auth_header[9..]; // Skip "X-Matrix "

    let mut origin = None;
    let mut key = None;
    let mut signature = None;

    for param in auth_params.split(',') {
        let param = param.trim();

        if let Some((key_name, value)) = param.split_once('=') {
            match key_name.trim() {
                "origin" => {
                    origin = Some(value.trim().to_string());
                },
                "key" => {
                    let key_value = value.trim().trim_matches('"');
                    if let Some(key_id) = key_value.strip_prefix("ed25519:") {
                        key = Some(key_id.to_string());
                    } else {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                },
                "sig" => {
                    signature = Some(value.trim().trim_matches('"').to_string());
                },
                _ => {
                    // Unknown parameter, ignore for forward compatibility
                },
            }
        }
    }

    let origin = origin.ok_or(StatusCode::BAD_REQUEST)?;
    let key_id = key.ok_or(StatusCode::BAD_REQUEST)?;
    let signature = signature.ok_or(StatusCode::BAD_REQUEST)?;

    Ok(XMatrixAuth { origin, key_id, signature })
}

/// GET /_matrix/federation/v1/state/{roomId}
///
/// Retrieves a snapshot of a room's state at a given event.
pub async fn get(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<StateQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
    })?;

    debug!(
        "State request - origin: {}, room: {}, event: {}",
        x_matrix_auth.origin, room_id, query.event_id
    );

    // Validate server signature
    let request_body = format!("room_id={}&event_id={}", room_id, query.event_id);
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            "/state",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate room exists and we know about it
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

    // Check if requesting server has permission to view room state
    let has_permission = check_state_permission(&state, &room, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check state permissions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !has_permission {
        warn!("Server {} not authorized to view state of room {}", x_matrix_auth.origin, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate the event exists in the room
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let target_event = event_repo
        .get_by_id(&query.event_id)
        .await
        .map_err(|e| {
            error!("Failed to query event {}: {}", query.event_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Event {} not found", query.event_id);
            StatusCode::NOT_FOUND
        })?;

    if target_event.room_id != room_id {
        warn!("Event {} is not in room {}", query.event_id, room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get room state at the specified event
    let room_state =
        get_room_state_at_event(&state, &room_id, &query.event_id)
            .await
            .map_err(|e| {
                error!("Failed to get room state: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    // Get auth chain for the state events
    let auth_chain = get_auth_chain_for_state(&state, &room_state).await.map_err(|e| {
        error!("Failed to get auth chain: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = json!({
        "pdus": room_state,
        "auth_chain": auth_chain
    });

    info!(
        "Retrieved state for room {} at event {} ({} state events, {} auth events)",
        room_id,
        query.event_id,
        room_state.len(),
        auth_chain.len()
    );

    Ok(Json(response))
}

/// Check if a server has permission to view room state
async fn check_state_permission(
    state: &AppState,
    room: &Room,
    requesting_server: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Check if the requesting server has any users in the room using repository
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let has_users = membership_repo
        .check_server_has_users_in_room(&room.room_id, requesting_server)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    if has_users {
        return Ok(true);
    }

    // Check if room is world-readable using repository
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let world_readable = event_repo
        .is_room_world_readable(&room.room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(world_readable)
}

/// Get room state at a specific event using repository
async fn get_room_state_at_event(
    state: &AppState,
    room_id: &str,
    event_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let state_events = event_repo
        .get_room_state_at_event(room_id, event_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(state_events)
}

/// Get auth chain for a set of state events using repository
async fn get_auth_chain_for_state(
    state: &AppState,
    state_events: &[Value],
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let auth_chain = event_repo
        .get_auth_chain_for_state(state_events)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(auth_chain)
}
