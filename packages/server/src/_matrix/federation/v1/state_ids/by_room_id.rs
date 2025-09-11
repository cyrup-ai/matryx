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

/// Query parameters for state_ids request
#[derive(Debug, Deserialize)]
pub struct StateIdsQuery {
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

/// GET /_matrix/federation/v1/state_ids/{roomId}
///
/// Retrieves just the state event IDs for a room at a given event,
/// without the full event content. More efficient than the full state endpoint.
pub async fn get(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<StateIdsQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "State IDs request - origin: {}, room: {}, event: {}",
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
            "/state_ids",
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

    // Get room state IDs at the specified event
    let state_event_ids = get_room_state_ids_at_event(&state, &room_id, &query.event_id)
        .await
        .map_err(|e| {
            error!("Failed to get room state IDs: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get auth chain IDs for the state events
    let auth_chain_ids =
        get_auth_chain_ids_for_state(&state, &state_event_ids).await.map_err(|e| {
            error!("Failed to get auth chain IDs: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response = json!({
        "pdu_ids": state_event_ids,
        "auth_chain_ids": auth_chain_ids
    });

    info!(
        "Retrieved state IDs for room {} at event {} ({} state event IDs, {} auth event IDs)",
        room_id,
        query.event_id,
        state_event_ids.len(),
        auth_chain_ids.len()
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

/// Get room state event IDs at a specific event
async fn get_room_state_ids_at_event(
    state: &AppState,
    room_id: &str,
    event_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
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

    // Get state event IDs at or before the target event depth
    let state_query = "
        SELECT event_id
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

    #[derive(serde::Deserialize)]
    struct EventId {
        event_id: String,
    }

    let events: Vec<EventId> = response.take(0)?;
    let state_event_ids: Vec<String> = events.into_iter().map(|e| e.event_id).collect();

    Ok(state_event_ids)
}

/// Get auth chain IDs for a set of state event IDs
async fn get_auth_chain_ids_for_state(
    state: &AppState,
    state_event_ids: &[String],
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    if state_event_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut auth_event_ids = HashSet::new();
    let mut to_process = HashSet::new();

    // Clone the state_event_ids to avoid lifetime issues
    let state_event_ids_owned: Vec<String> = state_event_ids.to_vec();

    // Get initial auth events from state events
    let query = "
        SELECT auth_events
        FROM event
        WHERE event_id IN $state_event_ids
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("state_event_ids", state_event_ids_owned))
        .await?;

    #[derive(serde::Deserialize)]
    struct AuthEvents {
        auth_events: Option<Vec<String>>,
    }

    let events: Vec<AuthEvents> = response.take(0)?;

    // Collect initial auth event IDs
    for event in events {
        if let Some(auth_events) = event.auth_events {
            for auth_event_id in auth_events {
                if auth_event_ids.insert(auth_event_id.clone()) {
                    to_process.insert(auth_event_id);
                }
            }
        }
    }

    // Recursively fetch auth events
    while !to_process.is_empty() {
        let current_batch: Vec<String> = to_process.drain().collect();

        let query = "
            SELECT auth_events
            FROM event
            WHERE event_id IN $auth_event_ids
        ";

        let mut response = state.db.query(query).bind(("auth_event_ids", current_batch)).await?;

        let events: Vec<AuthEvents> = response.take(0)?;

        // Process auth events of the fetched events
        for event in events {
            if let Some(auth_events) = event.auth_events {
                for auth_event_id in auth_events {
                    if auth_event_ids.insert(auth_event_id.clone()) {
                        to_process.insert(auth_event_id);
                    }
                }
            }
        }
    }

    let auth_chain_ids: Vec<String> = auth_event_ids.into_iter().collect();
    Ok(auth_chain_ids)
}
