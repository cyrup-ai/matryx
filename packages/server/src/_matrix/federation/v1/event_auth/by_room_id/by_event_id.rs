use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use tracing::{debug, error, info, warn};

use crate::AppState;
use matryx_surrealdb::repository::EventRepository;

/// GET /_matrix/federation/v1/event_auth/{roomId}/{eventId}
///
/// Retrieves the complete auth chain for a given event.
/// The auth chain includes all events that authorize the given event,
/// following the DAG structure recursively.
pub async fn get(
    State(state): State<AppState>,
    Path((room_id, event_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    info!("Fetching auth chain for event {} in room {}", event_id, room_id);

    // Validate room and event IDs
    if room_id.is_empty() || event_id.is_empty() {
        warn!("Invalid room_id or event_id provided");
        return Err(StatusCode::BAD_REQUEST);
    }

    let event_repo = EventRepository::new(state.db.clone());

    // Get the target event first
    let target_event = match event_repo.get_by_id(&event_id).await {
        Ok(Some(event)) => {
            if event.room_id != room_id {
                warn!("Event {} not found in room {}", event_id, room_id);
                return Err(StatusCode::NOT_FOUND);
            }
            event
        },
        Ok(None) => {
            warn!("Event {} not found", event_id);
            return Err(StatusCode::NOT_FOUND);
        },
        Err(e) => {
            error!("Database error fetching event {}: {}", event_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Build the complete auth chain using BFS traversal
    match build_auth_chain(&event_repo, &target_event).await {
        Ok(auth_chain) => {
            info!(
                "Successfully built auth chain with {} events for {}",
                auth_chain.len(),
                event_id
            );
            Ok(Json(json!({
                "auth_chain": auth_chain
            })))
        },
        Err(e) => {
            error!("Failed to build auth chain for event {}: {}", event_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

/// Build the complete authorization chain for an event
///
/// Uses breadth-first search to traverse the auth_events DAG and collect
/// all events that directly or indirectly authorize the target event.
async fn build_auth_chain(
    event_repo: &EventRepository<surrealdb::engine::any::Any>,
    target_event: &matryx_entity::types::Event,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let mut auth_chain = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    // Start with the target event's auth_events
    if let Some(auth_events) = &target_event.auth_events {
        for auth_event_id in auth_events {
            queue.push_back(auth_event_id.clone());
        }
    }

    // BFS traversal to collect all auth events
    while let Some(event_id) = queue.pop_front() {
        // Skip if already processed
        if visited.contains(&event_id) {
            continue;
        }
        visited.insert(event_id.clone());

        // Fetch the auth event
        match event_repo.get_by_id(&event_id).await? {
            Some(auth_event) => {
                // Convert event to JSON format for response
                let event_json = event_to_pdu_json(&auth_event)?;
                auth_chain.push(event_json);

                // Add this event's auth_events to the queue for further traversal
                if let Some(nested_auth_events) = &auth_event.auth_events {
                    for nested_auth_event_id in nested_auth_events {
                        if !visited.contains(nested_auth_event_id) {
                            queue.push_back(nested_auth_event_id.clone());
                        }
                    }
                }

                debug!("Added auth event {} to chain", event_id);
            },
            None => {
                warn!("Auth event {} not found in database", event_id);
                // Continue processing other auth events even if one is missing
            },
        }
    }

    debug!("Built auth chain with {} events", auth_chain.len());
    Ok(auth_chain)
}

/// Convert an Event entity to PDU JSON format for federation responses
fn event_to_pdu_json(event: &matryx_entity::types::Event) -> Result<Value, serde_json::Error> {
    let mut pdu = json!({
        "event_id": event.event_id,
        "room_id": event.room_id,
        "sender": event.sender,
        "type": event.event_type,
        "content": event.content,
        "origin_server_ts": event.origin_server_ts,
        "unsigned": event.unsigned.as_ref().cloned().unwrap_or_else(|| json!({}))
    });

    // Add optional fields if present
    if let Some(state_key) = &event.state_key {
        pdu["state_key"] = json!(state_key);
    }

    if let Some(auth_events) = &event.auth_events {
        pdu["auth_events"] = json!(auth_events);
    }

    if let Some(prev_events) = &event.prev_events {
        pdu["prev_events"] = json!(prev_events);
    }

    if let Some(depth) = event.depth {
        pdu["depth"] = json!(depth);
    }

    if let Some(hashes) = &event.hashes {
        pdu["hashes"] = serde_json::to_value(hashes)?;
    }

    if let Some(signatures) = &event.signatures {
        pdu["signatures"] = serde_json::to_value(signatures)?;
    }

    Ok(pdu)
}
