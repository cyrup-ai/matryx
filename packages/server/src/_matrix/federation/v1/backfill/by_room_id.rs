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
use matryx_entity::types::Room;
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

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
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
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
        
    // Validate room federation settings per Matrix specification
    if let Some(false) = room.federate {
        warn!(
            "Federation disabled for room {}, denying backfill request from {}",
            room_id, x_matrix_auth.origin
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate room version compatibility for backfill (Matrix spec compliance)
    let supported_versions = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"];
    if !supported_versions.contains(&room.room_version.as_str()) {
        warn!(
            "Unsupported room version {} for backfill in room {}, denying request from {}",
            room.room_version, room_id, x_matrix_auth.origin
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate Server ACL restrictions per Matrix spec Section 20
    let acl_validation_result = validate_server_acl(&state, &room, &x_matrix_auth.origin).await;
    match acl_validation_result {
        Ok(true) => {
            debug!("Server {} passed ACL validation for room {}", x_matrix_auth.origin, room_id);
        },
        Ok(false) => {
            warn!(
                "Server {} denied by room ACL for backfill in room {}",
                x_matrix_auth.origin, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        },
        Err(e) => {
            error!("Failed to validate server ACL for {}: {}", x_matrix_auth.origin, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Additional room-specific validation based on room properties
    validate_room_access_for_backfill(&room, &x_matrix_auth.origin)?;

    info!(
        "Processing backfill request for room {} (version: {}, federation: {}) from server {}",
        room_id,
        room.room_version,
        room.federate.unwrap_or(true),
        x_matrix_auth.origin
    );

    // Check if requesting server has permission to backfill
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let has_users = membership_repo.server_has_users_in_room(&room_id, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check server membership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let has_permission = if has_users {
        true
    } else {
        // Check if room is world-readable
        room_repo.is_room_world_readable(&room_id)
            .await
            .map_err(|e| {
                error!("Failed to check room world-readable status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    };

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
        let current_batch: Vec<String> = std::mem::take(&mut to_visit);

        // Fetch current batch of events
        let event_repo = EventRepository::new(state.db.clone());
        let events = event_repo
            .get_events_by_ids_for_backfill(&current_batch, room_id)
            .await?;

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

/// Validate server against room's Server ACL restrictions per Matrix specification
///
/// Implements Matrix Server ACL validation according to the Matrix Server-Server API
/// specification Section 20. Server ACLs enable room administrators to control
/// which homeservers can participate in federation for specific rooms.
async fn validate_server_acl(
    state: &AppState,
    room: &Room,
    requesting_server: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Query for m.room.server_acl state event in the room
    let event_repo = EventRepository::new(state.db.clone());
    let acl_event = event_repo
        .get_server_acl_event(&room.room_id)
        .await?;

    let acl_event = match acl_event {
        Some(event) => event,
        None => {
            // No Server ACL configured - allow all servers
            debug!("No Server ACL found for room {}, allowing server {}", room.room_id, requesting_server);
            return Ok(true);
        }
    };
    let acl_content = acl_event.content.as_object()
        .ok_or("Server ACL content is not an object")?;

    // Get allow and deny lists from the ACL content
    let allow_list = acl_content
        .get("allow")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<&str>>()
        })
        .unwrap_or_else(|| vec!["*"]); // Default: allow all

    let deny_list = acl_content
        .get("deny")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<&str>>()
        })
        .unwrap_or_default();

    let allow_ip_literals = acl_content
        .get("allow_ip_literals")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Check if server is an IP literal and IP literals are denied
    if !allow_ip_literals && is_ip_literal(requesting_server) {
        debug!(
            "Server {} is IP literal and IP literals are not allowed in room {}",
            requesting_server, room.room_id
        );
        return Ok(false);
    }

    // Check deny list first (takes precedence)
    for deny_pattern in &deny_list {
        if matches_server_pattern(requesting_server, deny_pattern) {
            debug!(
                "Server {} matches deny pattern '{}' in room {}",
                requesting_server, deny_pattern, room.room_id
            );
            return Ok(false);
        }
    }

    // Check allow list
    for allow_pattern in &allow_list {
        if matches_server_pattern(requesting_server, allow_pattern) {
            debug!(
                "Server {} matches allow pattern '{}' in room {}",
                requesting_server, allow_pattern, room.room_id
            );
            return Ok(true);
        }
    }

    // If no allow patterns matched, deny access
    debug!(
        "Server {} does not match any allow patterns in room {}",
        requesting_server, room.room_id
    );
    Ok(false)
}

/// Check if a server name is an IP literal
fn is_ip_literal(server_name: &str) -> bool {
    use std::net::IpAddr;

    // Split server name to handle port numbers
    let host_part = server_name.split(':').next().unwrap_or(server_name);

    // Remove brackets for IPv6 literals
    let host_clean = host_part.trim_start_matches('[').trim_end_matches(']');

    // Try parsing as IP address
    host_clean.parse::<IpAddr>().is_ok()
}

/// Check if a server name matches a Server ACL pattern
///
/// Implements Matrix Server ACL pattern matching:
/// - '*' matches any sequence of characters
/// - '?' matches any single character
/// - Literal characters must match exactly
fn matches_server_pattern(server_name: &str, pattern: &str) -> bool {
    // Convert pattern to regex-like matching
    let mut regex_pattern = String::new();
    let chars = pattern.chars().peekable();

    for ch in chars {
        match ch {
            '*' => regex_pattern.push_str(".*"),
            '?' => regex_pattern.push('.'),
            // Escape special regex characters
            '.' | '+' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '|' => {
                regex_pattern.push('\\');
                regex_pattern.push(ch);
            }
            _ => regex_pattern.push(ch),
        }
    }

    // Use simple pattern matching for now - in production would use regex crate
    // For simplicity, implement basic wildcard matching
    simple_wildcard_match(server_name, pattern)
}

/// Simple wildcard pattern matching for Server ACL patterns
fn simple_wildcard_match(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();

    wildcard_match_recursive(&text_chars, &pattern_chars, 0, 0)
}

/// Recursive wildcard matching implementation
fn wildcard_match_recursive(
    text: &[char],
    pattern: &[char],
    text_idx: usize,
    pattern_idx: usize
) -> bool {
    // Base cases
    if pattern_idx == pattern.len() {
        return text_idx == text.len();
    }

    if text_idx == text.len() {
        // Check if remaining pattern is all '*'
        return pattern[pattern_idx..].iter().all(|&c| c == '*');
    }

    match pattern[pattern_idx] {
        '*' => {
            // Try matching zero characters or more
            wildcard_match_recursive(text, pattern, text_idx, pattern_idx + 1) ||
            wildcard_match_recursive(text, pattern, text_idx + 1, pattern_idx)
        },
        '?' => {
            // Match any single character
            wildcard_match_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
        },
        c if c == text[text_idx] => {
            // Exact character match
            wildcard_match_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
        },
        _ => false, // No match
    }
}

/// Additional room access validation for backfill requests
///
/// Performs Matrix specification-compliant validation of room access
/// based on room properties and requesting server characteristics.
fn validate_room_access_for_backfill(
    room: &Room,
    requesting_server: &str,
) -> Result<(), StatusCode> {
    // Validate room is not tombstoned (room version upgrade scenario)
    if let Some(tombstone) = &room.tombstone
        && tombstone.get("replacement_room").is_some() {
        warn!(
            "Room {} is tombstoned, denying backfill request from {}",
            room.room_id, requesting_server
        );
        return Err(StatusCode::GONE); // HTTP 410 Gone for tombstoned rooms
    }

    // Validate room type restrictions for backfill
    if let Some(room_type) = &room.room_type {
        match room_type.as_str() {
            "m.space" => {
                // Spaces may have different backfill policies
                debug!(
                    "Backfill request for space room {} from server {}",
                    room.room_id, requesting_server
                );
            },
            other_type => {
                // Unknown room types - be cautious but allow
                debug!(
                    "Backfill request for room type '{}' in room {} from server {}",
                    other_type, room.room_id, requesting_server
                );
            }
        }
    }

    // Validate history visibility for federation backfill
    match room.history_visibility.as_deref() {
        Some("world_readable") => {
            // World readable rooms allow backfill from any server
            debug!(
                "World readable room {}, allowing backfill from {}",
                room.room_id, requesting_server
            );
        },
        Some("shared") | Some("invited") | Some("joined") | None => {
            // These visibility levels require membership validation (handled elsewhere)
            debug!(
                "Room {} has visibility '{}', membership validation required",
                room.room_id, room.history_visibility.as_deref().unwrap_or("default")
            );
        },
        Some(unknown) => {
            warn!(
                "Unknown history visibility '{}' for room {}, allowing with caution",
                unknown, room.room_id
            );
        }
    }

    debug!(
        "Room access validation passed for backfill: room={}, server={}",
        room.room_id, requesting_server
    );
    Ok(())
}
