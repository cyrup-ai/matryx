use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{Event, Room};
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

/// Query parameters for backfill request
#[derive(Debug, Deserialize)]
pub struct BackfillQuery {
    /// The maximum number of PDUs to retrieve, including the given events
    limit: i32,
    /// The event IDs to backfill from
    v: Vec<String>,
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

/// GET /_matrix/federation/v1/backfill/{roomId}
///
/// Retrieves a sliding-window history of previous PDUs that occurred in the given room.
pub async fn get(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<BackfillQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "Backfill request - origin: {}, room: {}, limit: {}, from: {:?}",
        x_matrix_auth.origin, room_id, query.limit, query.v
    );

    // Validate server signature
    let request_body =
        format!("room_id={}&limit={}&v={}", room_id, query.limit, query.v.join("&v="));
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            "/backfill",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate parameters
    if query.limit <= 0 || query.limit > 100 {
        warn!("Invalid backfill limit: {}", query.limit);
        return Err(StatusCode::BAD_REQUEST);
    }

    if query.v.is_empty() {
        warn!("No starting events provided for backfill");
        return Err(StatusCode::BAD_REQUEST);
    }

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

    // Check if requesting server has permission to backfill
    let has_permission = check_backfill_permission(&state, &room, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check backfill permissions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !has_permission {
        warn!("Server {} not authorized to backfill room {}", x_matrix_auth.origin, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate starting events exist in the room
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    for event_id in &query.v {
        let event = event_repo
            .get_by_id(event_id)
            .await
            .map_err(|e| {
                error!("Failed to query event {}: {}", event_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or_else(|| {
                warn!("Starting event {} not found", event_id);
                StatusCode::NOT_FOUND
            })?;

        if event.room_id != room_id {
            warn!("Starting event {} is not in room {}", event_id, room_id);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Perform backfill traversal
    let backfilled_events = backfill_events(&state, &room_id, &query.v, query.limit as usize)
        .await
        .map_err(|e| {
            error!("Failed to backfill events: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response = json!({
        "origin": state.homeserver_name,
        "origin_server_ts": Utc::now().timestamp_millis(),
        "pdus": backfilled_events
    });

    info!(
        "Backfilled {} events for room {} from server {}",
        backfilled_events.len(),
        room_id,
        x_matrix_auth.origin
    );

    Ok(Json(response))
}

/// Check if a server has permission to backfill a room
async fn check_backfill_permission(
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

/// Backfill events using breadth-first traversal
async fn backfill_events(
    state: &AppState,
    room_id: &str,
    starting_event_ids: &[String],
    limit: usize,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let mut visited = HashSet::new();
    let mut to_visit: Vec<String> = starting_event_ids.to_vec();
    let mut result_events = Vec::new();

    // Add starting events to visited set
    for event_id in starting_event_ids {
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
            ORDER BY depth DESC, origin_server_ts DESC
        ";

        let mut response = state
            .db
            .query(query)
            .bind(("event_ids", current_batch.clone()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let events: Vec<Event> = response.take(0)?;

        // Add events to result
        for event in events {
            if result_events.len() >= limit {
                break;
            }

            let event_json = serde_json::to_value(&event)?;
            result_events.push(event_json);

            // Add prev_events to the next batch if not visited
            if let Some(prev_events) = &event.prev_events {
                for prev_event_id in prev_events {
                    if !visited.contains(prev_event_id) {
                        visited.insert(prev_event_id.to_string());
                        to_visit.push(prev_event_id.to_string());
                    }
                }
            }
        }
    }

    // Sort by depth descending (most recent first) then by origin_server_ts
    result_events.sort_by(|a, b| {
        let depth_a = a.get("depth").and_then(|v| v.as_i64()).unwrap_or(0);
        let depth_b = b.get("depth").and_then(|v| v.as_i64()).unwrap_or(0);

        match depth_b.cmp(&depth_a) {
            std::cmp::Ordering::Equal => {
                let ts_a = a.get("origin_server_ts").and_then(|v| v.as_i64()).unwrap_or(0);
                let ts_b = b.get("origin_server_ts").and_then(|v| v.as_i64()).unwrap_or(0);
                ts_b.cmp(&ts_a)
            },
            other => other,
        }
    });

    Ok(result_events)
}
