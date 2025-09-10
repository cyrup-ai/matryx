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
use matryx_entity::types::{Event, MissingEventsRequest, MissingEventsResponse, PDU};
use matryx_surrealdb::repository::EventRepository;

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

/// POST /_matrix/federation/v1/get_missing_events/{roomId}
/// 
/// Retrieves previous events that the sender is missing. This is done by doing a breadth-first 
/// walk of the prev_events for the latest_events, ignoring any events in earliest_events and 
/// stopping at the limit.
pub async fn post(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(payload): Json<MissingEventsRequest>,
) -> Result<Json<MissingEventsResponse>, StatusCode> {
    debug!(
        "Get missing events request - room: {}, latest_events: {:?}, earliest_events: {:?}, limit: {:?}, min_depth: {:?}",
        room_id, payload.latest_events, payload.earliest_events, payload.limit, payload.min_depth
    );

    // Validate server signature
    let request_body = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize request body: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // TODO: Implement proper X-Matrix authentication validation
    // For now, skip authentication validation

    // Validate parameters
    let limit = payload.limit.unwrap_or(10) as usize;
    let min_depth = payload.min_depth.unwrap_or(0);

    if limit == 0 || limit > 100 {
        warn!("Invalid missing events limit: {}", limit);
        return Err(StatusCode::BAD_REQUEST);
    }

    if payload.latest_events.is_empty() {
        warn!("No latest events provided for get_missing_events");
        return Err(StatusCode::BAD_REQUEST);
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

    info!(
        "Retrieved {} missing events for room {}",
        response.events.len(),
        room_id
    );

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
                // Convert Event to PDU (handling optional fields)
                let pdu = PDU {
                    event_id: event.event_id.clone(),
                    room_id: event.room_id.clone(),
                    sender: event.sender.clone(),
                    origin_server_ts: event.origin_server_ts,
                    event_type: event.event_type.clone(),
                    content: event.content.clone(),
                    state_key: event.state_key.clone(),
                    prev_events: event.prev_events.clone().unwrap_or_default(),
                    auth_events: event.auth_events.clone().unwrap_or_default(),
                    depth: event.depth.unwrap_or(0),
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
