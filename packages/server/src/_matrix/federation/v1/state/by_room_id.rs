use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{Event, Room};
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

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
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
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
    // Check if the requesting server has any users in the room
    let query = "
        SELECT COUNT() as count
        FROM membership
        WHERE room_id = $room_id
        AND user_id CONTAINS $server_suffix
        AND membership IN ['join', 'invite', 'leave']
        LIMIT 1
    ";

    let server_suffix = format!(":{}", requesting_server);

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room.room_id.clone()))
        .bind(("server_suffix", server_suffix))
        .await?;

    #[derive(serde::Deserialize)]
    struct CountResult {
        count: i64,
    }

    let count_result: Option<CountResult> = response.take(0)?;
    let has_users = count_result.map(|c| c.count > 0).unwrap_or(false);

    if has_users {
        return Ok(true);
    }

    // Check if room is world-readable
    let world_readable = is_room_world_readable(state, &room.room_id).await?;
    Ok(world_readable)
}

/// Check if a room is world-readable
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

/// Get room state at a specific event
async fn get_room_state_at_event(
    state: &AppState,
    room_id: &str,
    event_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the target event's depth for state resolution
    let depth_query = "
        SELECT depth
        FROM event
        WHERE event_id = $event_id
    ";

    let mut response = state
        .db
        .query(depth_query)
        .bind(("event_id", event_id.to_string()))
        .await?;

    #[derive(serde::Deserialize)]
    struct EventDepth {
        depth: i64,
    }

    let event_depth: Option<EventDepth> = response.take(0)?;
    let target_depth = event_depth.map(|e| e.depth).ok_or("Event depth not found")?;

    // Get state events at or before the target event depth
    let state_query = "
        SELECT *
        FROM event
        WHERE room_id = $room_id
        AND state_key IS NOT NULL
        AND depth <= $target_depth
        AND (
            SELECT COUNT()
            FROM event e2
            WHERE e2.room_id = $room_id
            AND e2.type = event.type
            AND e2.state_key = event.state_key
            AND e2.depth <= $target_depth
            AND (e2.depth > event.depth OR (e2.depth = event.depth AND e2.origin_server_ts > event.origin_server_ts))
        ) = 0
        ORDER BY type, state_key
    ";

    let mut response = state
        .db
        .query(state_query)
        .bind(("room_id", room_id.to_string()))
        .bind(("target_depth", target_depth))
        .await?;

    let events: Vec<Event> = response.take(0)?;

    // Convert events to JSON format for response
    let state_events: Vec<Value> = events
        .into_iter()
        .map(|event| serde_json::to_value(event).unwrap_or(json!({})))
        .collect();

    Ok(state_events)
}

/// Get auth chain for a set of state events
async fn get_auth_chain_for_state(
    state: &AppState,
    state_events: &[Value],
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let mut auth_event_ids = HashSet::new();
    let mut to_process = HashSet::new();

    // Collect initial auth events
    for state_event in state_events {
        if let Some(auth_events) = state_event.get("auth_events").and_then(|v| v.as_array()) {
            for auth_event in auth_events {
                if let Some(auth_event_id) = auth_event.as_str() {
                    if auth_event_ids.insert(auth_event_id.to_string()) {
                        to_process.insert(auth_event_id.to_string());
                    }
                }
            }
        }
    }

    // Recursively fetch auth events
    while !to_process.is_empty() {
        let current_batch: Vec<String> = to_process.drain().collect();

        let query = "
            SELECT *
            FROM event
            WHERE event_id IN $auth_event_ids
        ";

        let mut response = state.db.query(query).bind(("auth_event_ids", current_batch)).await?;

        let events: Vec<Event> = response.take(0)?;

        // Process auth events of the fetched events
        for event in &events {
            if let Some(auth_events) = &event.auth_events {
                for auth_event_id in auth_events {
                    if auth_event_ids.insert(auth_event_id.clone()) {
                        to_process.insert(auth_event_id.clone());
                    }
                }
            }
        }
    }

    if auth_event_ids.is_empty() {
        return Ok(vec![]);
    }

    // Fetch all auth events
    let auth_ids: Vec<String> = auth_event_ids.into_iter().collect();

    let query = "
        SELECT *
        FROM event
        WHERE event_id IN $auth_event_ids
        ORDER BY depth, origin_server_ts
    ";

    let mut response = state.db.query(query).bind(("auth_event_ids", auth_ids)).await?;

    let events: Vec<Event> = response.take(0)?;

    // Convert events to JSON format for response
    let auth_chain: Vec<Value> = events
        .into_iter()
        .map(|event| serde_json::to_value(event).unwrap_or(json!({})))
        .collect();

    Ok(auth_chain)
}
