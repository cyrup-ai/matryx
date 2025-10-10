use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};

use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::{MissingEventsRequest, MissingEventsResponse, PDU, Room};
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository, UserRepository};

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

/// Validate room version compatibility for get_missing_events
fn validate_room_version_compatibility(room: &Room) -> Result<(), StatusCode> {
    // Matrix room versions 1-11 support get_missing_events
    let supported_versions = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"];

    if !supported_versions.contains(&room.room_version.as_str()) {
        warn!("Unsupported room version {} for get_missing_events", room.room_version);
        return Err(StatusCode::BAD_REQUEST);
    }

    debug!("Room version {} is compatible with get_missing_events", room.room_version);
    Ok(())
}

/// Validate federation access based on room settings
fn validate_federation_access(room: &Room, requesting_server: &str) -> Result<(), StatusCode> {
    // Check if room federation is disabled
    if let Some(false) = room.federate {
        warn!(
            "Federation disabled for room {}, denying access to {}",
            room.room_id, requesting_server
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // For invite-only rooms, additional checks could be implemented here
    if let Some(join_rule) = &room.join_rule
        && join_rule == "invite"
    {
        debug!(
            "Room {} is invite-only, federation access granted to {}",
            room.room_id, requesting_server
        );
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
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
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

    // Validate limit bounds (reduced to 20 to match Synapse for conservative data transfer)
    if limit == 0 || limit > 20 {
        warn!("Invalid missing events limit: {} (max 20)", limit);
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

    validate_event_id_list(&payload.latest_events, 50, "latest_events").inspect_err(|_e| {
        warn!("Invalid latest_events list");
    })?;

    // Validate earliest_events list (can be empty)
    validate_event_id_list(&payload.earliest_events, 50, "earliest_events").inspect_err(|_e| {
        warn!("Invalid earliest_events list");
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

    // Validate room version for get_missing_events compatibility
    validate_room_version_compatibility(&room)?;

    // Apply room-specific federation rules
    validate_federation_access(&room, &x_matrix_auth.origin)?;

    // Check if requesting server has permission to access room
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let has_users = membership_repo
        .server_has_users_in_room(&room_id, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check server membership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let has_permission = if has_users {
        true
    } else {
        // Check if room is world-readable
        room_repo.is_room_world_readable(&room_id).await.map_err(|e| {
            error!("Failed to check room world-readable status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

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

    // Apply event visibility filtering for security
    let filtered_events = filter_events_for_server(
        &state,
        &room,
        &x_matrix_auth.origin,
        missing_events,
        has_users,
    )
    .await
    .map_err(|e| {
        error!("Failed to filter events: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = MissingEventsResponse::new(filtered_events);

    info!("Retrieved {} missing events for room {}", response.events.len(), room_id);

    Ok(Json(response))
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
    let mut visited: HashSet<String> = HashSet::new();
    let mut to_visit: Vec<String> = Vec::new();
    let mut result_events = Vec::new();
    let earliest_set: HashSet<String> = earliest_events.iter().cloned().collect();

    // Mark latest_events as visited (they are starting points only, NOT results)
    for event_id in latest_events {
        visited.insert(event_id.clone());
    }

    // STEP 1: Fetch latest_events to get their prev_events (don't add to results)
    let event_repo = EventRepository::new(state.db.clone());
    let start_events = event_repo
        .get_events_by_ids_with_min_depth(latest_events, room_id, min_depth)
        .await?;

    // Extract prev_events from latest_events as the true starting point for BFS
    for event in start_events {
        if let Some(prev_events) = &event.prev_events {
            for prev_event_id in prev_events {
                if !visited.contains(prev_event_id) && !earliest_set.contains(prev_event_id) {
                    visited.insert(prev_event_id.to_string());
                    to_visit.push(prev_event_id.to_string());
                }
            }
        }
    }

    // STEP 2: BFS traversal starting from prev_events (these DO get added to results)
    while !to_visit.is_empty() && result_events.len() < limit {
        let current_batch: Vec<String> = std::mem::take(&mut to_visit);

        // Fetch current batch of events
        let events = event_repo
            .get_events_by_ids_with_min_depth(&current_batch, room_id, min_depth)
            .await?;

        // Process events and add their prev_events to next batch
        for event in events {
            // Skip if this event is in earliest_events boundary
            if earliest_set.contains(&event.event_id) {
                continue;
            }

            // Queue this event's prev_events for next iteration
            if let Some(prev_events) = &event.prev_events {
                for prev_event_id in prev_events {
                    if !visited.contains(prev_event_id) && !earliest_set.contains(prev_event_id) {
                        visited.insert(prev_event_id.to_string());
                        to_visit.push(prev_event_id.to_string());
                    }
                }
            }

            // Add event to results (NOW this is correct - latest_events are excluded)
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
    result_events.sort_by(|a, b| match b.depth.cmp(&a.depth) {
        std::cmp::Ordering::Equal => b.origin_server_ts.cmp(&a.origin_server_ts),
        other => other,
    });

    Ok(result_events)
}

/// Filter events based on room history visibility settings and advanced security features
///
/// This function implements comprehensive server-side event visibility filtering with:
/// - Basic history visibility filtering (world_readable, shared, invited, joined)
/// - GDPR compliance: filters events from erased users (right to be forgotten)
/// - Partial state protection: filters remote events when room is in partial state
/// - Per-event membership verification: checks server membership at event depth
///
/// # Arguments
/// * `state` - Application state for database access
/// * `room` - The room containing the events
/// * `requesting_server` - The origin server making the request
/// * `events` - The list of events to filter
/// * `server_has_users` - Whether the requesting server has users in the room currently
///
/// # Returns
/// Filtered list of events the requesting server is authorized to see
async fn filter_events_for_server(
    state: &AppState,
    room: &Room,
    requesting_server: &str,
    events: Vec<PDU>,
    server_has_users: bool,
) -> Result<Vec<PDU>, Box<dyn std::error::Error + Send + Sync>> {
    // Check room history visibility setting
    let history_visibility = room.history_visibility.as_deref().unwrap_or("shared");

    // Basic history visibility filtering
    let events = match history_visibility {
        "world_readable" => {
            // World-readable rooms: all events visible to everyone
            debug!(
                "Room {} is world_readable, allowing all {} events for server {}",
                room.room_id,
                events.len(),
                requesting_server
            );
            events
        },
        "shared" | "invited" | "joined" => {
            // Non-world-readable rooms: only servers with users can see events
            if server_has_users {
                debug!(
                    "Server {} has users in room {} (history_visibility: {}), allowing all {} events",
                    requesting_server,
                    room.room_id,
                    history_visibility,
                    events.len()
                );
                events
            } else {
                // Server has no users in room, deny all events
                warn!(
                    "Server {} has no users in room {} (history_visibility: {}), filtering all events",
                    requesting_server,
                    room.room_id,
                    history_visibility
                );
                return Ok(Vec::new());
            }
        },
        _ => {
            // Unknown history_visibility setting, default to restrictive behavior
            warn!(
                "Unknown history_visibility '{}' for room {}, applying restrictive filtering",
                history_visibility,
                room.room_id
            );
            if server_has_users {
                events
            } else {
                return Ok(Vec::new());
            }
        },
    };

    // FEATURE A: Erased senders filtering (GDPR compliance)
    // Check if any event senders have been erased and filter those events
    let user_repo = UserRepository::new(state.db.clone());
    let sender_list: Vec<String> = events.iter().map(|e| e.sender.clone()).collect();
    let erased_senders = user_repo.are_users_erased(&sender_list).await?;

    let mut filtered_events = Vec::new();
    let mut erased_count = 0;

    for event in events {
        if let Some(&is_erased) = erased_senders.get(&event.sender) {
            if is_erased {
                info!(
                    "Sender {} of event {} has been erased, redacting event for GDPR compliance",
                    event.sender, event.event_id
                );
                erased_count += 1;
                continue; // Skip this event
            }
        }
        filtered_events.push(event);
    }

    if erased_count > 0 {
        info!(
            "Filtered {} events from erased senders in room {} for server {}",
            erased_count, room.room_id, requesting_server
        );
    }

    // FEATURE B: Partial state events filtering
    // Filter out events from remote servers when room is in partial state
    let room_repo = RoomRepository::new(state.db.clone());
    let is_partial_state = room_repo.is_partial_state_room(&room.room_id).await?;

    if is_partial_state {
        let homeserver_name = &state.homeserver_name;
        let mut partial_state_filtered_events = Vec::new();
        let mut partial_state_count = 0;

        for event in filtered_events {
            // Extract sender domain
            let sender_domain = if let Some(colon_pos) = event.sender.rfind(':') {
                &event.sender[colon_pos + 1..]
            } else {
                // Invalid sender format, skip for safety
                warn!(
                    "Invalid sender format in event {}: {}",
                    event.event_id, event.sender
                );
                partial_state_count += 1;
                continue;
            };

            // Filter out non-local events when room is in partial state
            if sender_domain != homeserver_name {
                debug!(
                    "Filtering event {} from remote server {} (room {} is in partial state)",
                    event.event_id, sender_domain, room.room_id
                );
                partial_state_count += 1;
                continue;
            }

            partial_state_filtered_events.push(event);
        }

        if partial_state_count > 0 {
            info!(
                "Filtered {} remote events from partial state room {} for server {}",
                partial_state_count, room.room_id, requesting_server
            );
        }

        filtered_events = partial_state_filtered_events;
    }

    // FEATURE C: Per-event membership verification
    // Verify server had members at each event's specific depth for more precise access control
    let membership_repo = MembershipRepository::new(state.db.clone());
    let mut depth_filtered_events = Vec::new();
    let mut depth_filtered_count = 0;

    for event in filtered_events {
        // Check if requesting server had users at this event's depth
        let had_members_at_depth = membership_repo
            .get_server_membership_at_depth(&room.room_id, requesting_server, event.depth)
            .await?;

        if !had_members_at_depth {
            debug!(
                "Server {} had no members at depth {} for event {}, filtering",
                requesting_server, event.depth, event.event_id
            );
            depth_filtered_count += 1;
            continue;
        }

        depth_filtered_events.push(event);
    }

    if depth_filtered_count > 0 {
        info!(
            "Filtered {} events based on per-depth membership verification in room {} for server {}",
            depth_filtered_count, room.room_id, requesting_server
        );
    }

    Ok(depth_filtered_events)
}
