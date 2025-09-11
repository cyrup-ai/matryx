use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{Event, MissingEventsRequest, MissingEventsResponse, PDU, Room};
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

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

/// Validate Matrix room ID format
fn validate_room_id(room_id: &str) -> Result<(), StatusCode> {
    if !room_id.starts_with('!') || !room_id.contains(':') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Validate Matrix event ID format  
fn validate_event_id(event_id: &str) -> Result<(), StatusCode> {
    if !event_id.starts_with('$') || !event_id.contains(':') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Validate event ID list for size and format
fn validate_event_id_list(
    event_ids: &[String],
    max_size: usize,
    list_name: &str,
) -> Result<(), StatusCode> {
    if event_ids.len() > max_size {
        warn!("{} list too large: {} events (max {})", list_name, event_ids.len(), max_size);
        return Err(StatusCode::BAD_REQUEST);
    }

    for event_id in event_ids {
        validate_event_id(event_id).map_err(|_| {
            warn!("Invalid event ID format in {}: {}", list_name, event_id);
            StatusCode::BAD_REQUEST
        })?;
    }

    // Check for duplicate event IDs
    let mut seen = HashSet::with_capacity(event_ids.len());
    for event_id in event_ids {
        if !seen.insert(event_id) {
            warn!("Duplicate event ID in {}: {}", list_name, event_id);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    Ok(())
}

/// POST /_matrix/federation/v1/get_missing_events/{roomId}
///
/// Retrieves previous events that the sender is missing. This is done by doing a breadth-first
/// walk of the prev_events for the latest_events, ignoring any events in earliest_events and
/// stopping at the limit.
pub async fn post(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<MissingEventsRequest>,
) -> Result<Json<MissingEventsResponse>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "Get missing events request - origin: {}, room: {}, latest_events: {:?}, earliest_events: {:?}, limit: {:?}, min_depth: {:?}",
        x_matrix_auth.origin,
        room_id,
        payload.latest_events,
        payload.earliest_events,
        payload.limit,
        payload.min_depth
    );

    // Validate server signature
    let request_body = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize request body: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "POST",
            &format!("/_matrix/federation/v1/get_missing_events/{}", room_id),
            &request_body,
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate input parameters
    let limit = payload.limit.unwrap_or(10) as usize;
    let min_depth = payload.min_depth.unwrap_or(0);

    // Validate limit bounds
    if limit == 0 || limit > 100 {
        warn!("Invalid missing events limit: {}", limit);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate min_depth is non-negative
    if min_depth < 0 {
        warn!("Invalid min_depth: {} (must be >= 0)", min_depth);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate room ID format
    validate_room_id(&room_id).map_err(|_| {
        warn!("Invalid room ID format: {}", room_id);
        StatusCode::BAD_REQUEST
    })?;

    // Validate latest_events list
    if payload.latest_events.is_empty() {
        warn!("No latest events provided for get_missing_events");
        return Err(StatusCode::BAD_REQUEST);
    }

    validate_event_id_list(&payload.latest_events, 50, "latest_events").map_err(|e| {
        warn!("Invalid latest_events list");
        e
    })?;

    // Validate earliest_events list (can be empty)
    validate_event_id_list(&payload.earliest_events, 50, "earliest_events").map_err(|e| {
        warn!("Invalid earliest_events list");
        e
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

    // Check if requesting server has permission to access room
    let has_permission = check_missing_events_permission(&state, &room, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check missing events permissions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !has_permission {
        warn!(
            "Server {} not authorized to access missing events for room {}",
            x_matrix_auth.origin, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate latest events exist in the room
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    for event_id in &payload.latest_events {
        let event = event_repo
            .get_by_id(event_id)
            .await
            .map_err(|e| {
                error!("Failed to query event {}: {}", event_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or_else(|| {
                warn!("Latest event {} not found", event_id);
                StatusCode::NOT_FOUND
            })?;

        if event.room_id != room_id {
            warn!("Latest event {} is not in room {}", event_id, room_id);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Perform missing events traversal
    let missing_events = get_missing_events_traversal(
        &state,
        &room_id,
        &payload.latest_events,
        &payload.earliest_events,
        limit,
        min_depth,
    )
    .await
    .map_err(|e| {
        error!("Failed to retrieve missing events: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = MissingEventsResponse::new(missing_events);

    info!("Retrieved {} missing events for room {}", response.events.len(), room_id);

    Ok(Json(response))
}

/// Check if a server has permission to access missing events for a room
async fn check_missing_events_permission(
    state: &AppState,
    room: &Room,
    requesting_server: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Check if the requesting server has any users in the room (current or historical)
    let query = "
        SELECT COUNT() as count
        FROM membership
        WHERE room_id = $room_id
        AND user_id CONTAINS $server_suffix
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

/// Retrieve missing events using breadth-first traversal
async fn get_missing_events_traversal(
    state: &AppState,
    room_id: &str,
    latest_events: &[String],
    earliest_events: &[String],
    limit: usize,
    min_depth: i64,
) -> Result<Vec<PDU>, Box<dyn std::error::Error + Send + Sync>> {
    let mut visited = HashSet::new();
    let mut to_visit: Vec<String> = latest_events.to_vec();
    let mut result_events = Vec::new();
    let earliest_set: HashSet<String> = earliest_events.iter().cloned().collect();

    // Add latest events to visited set (they are starting points, not results)
    for event_id in latest_events {
        visited.insert(event_id.clone());
    }

    while !to_visit.is_empty() && result_events.len() < limit {
        let current_batch: Vec<String> = to_visit.drain(..).collect();

        // Fetch current batch of events
        let query = "
            SELECT *
            FROM event
            WHERE event_id IN $event_ids
            AND room_id = $room_id
            AND depth >= $min_depth
            ORDER BY depth DESC, origin_server_ts DESC
        ";

        let mut response = state
            .db
            .query(query)
            .bind(("event_ids", current_batch.clone()))
            .bind(("room_id", room_id.to_string()))
            .bind(("min_depth", min_depth))
            .await?;

        let events: Vec<Event> = response.take(0)?;

        // Process events and add their prev_events to next batch
        for event in events {
            // Skip if this event is in earliest_events
            if earliest_set.contains(&event.event_id) {
                continue;
            }

            // Add prev_events to the next batch if not visited and not in earliest_events
            if let Some(prev_events) = &event.prev_events {
                for prev_event_id in prev_events {
                    if !visited.contains(prev_event_id) && !earliest_set.contains(prev_event_id) {
                        visited.insert(prev_event_id.to_string());
                        to_visit.push(prev_event_id.to_string());
                    }
                }
            }

            // Add event to results if we haven't reached limit
            if result_events.len() < limit {
                // Validate required fields exist
                let depth = event.depth.ok_or("Event missing required depth field")?;
                let prev_events = event.prev_events.clone().unwrap_or_default();
                let auth_events = event.auth_events.clone().unwrap_or_default();

                // Convert Event to PDU (handling optional fields safely)
                let pdu = PDU {
                    event_id: event.event_id.clone(),
                    room_id: event.room_id.clone(),
                    sender: event.sender.clone(),
                    origin_server_ts: event.origin_server_ts,
                    event_type: event.event_type.clone(),
                    content: event.content.clone(),
                    state_key: event.state_key.clone(),
                    prev_events,
                    auth_events,
                    depth,
                    signatures: event.signatures.clone().unwrap_or_default(),
                    hashes: event.hashes.clone().unwrap_or_default(),
                    unsigned: event.unsigned.clone().and_then(|v| serde_json::from_value(v).ok()),
                };
                result_events.push(pdu);
            }
        }
    }

    // Sort by depth descending (most recent first) then by origin_server_ts
    result_events.sort_by(|a, b| {
        match b.depth.cmp(&a.depth) {
            std::cmp::Ordering::Equal => b.origin_server_ts.cmp(&a.origin_server_ts),
            other => other,
        }
    });

    Ok(result_events)
}
