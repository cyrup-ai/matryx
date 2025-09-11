use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{Event, PDU, Transaction};
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

/// Validate Matrix event ID format
fn validate_event_id(event_id: &str) -> Result<(), StatusCode> {
    if !event_id.starts_with('$') || !event_id.contains(':') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// GET /_matrix/federation/v1/event/{eventId}
///
/// Retrieves a single event. Returns a transaction containing a single PDU
/// which is the event requested.
pub async fn get(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Transaction>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!("Event retrieval request - origin: {}, event: {}", x_matrix_auth.origin, event_id);

    // Validate server signature
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            &format!("/_matrix/federation/v1/event/{}", event_id),
            &[],
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate event ID format
    validate_event_id(&event_id).map_err(|_| {
        warn!("Invalid event ID format: {}", event_id);
        StatusCode::BAD_REQUEST
    })?;

    // Retrieve event from database
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let event = event_repo
        .get_by_id(&event_id)
        .await
        .map_err(|e| {
            error!("Failed to query event {}: {}", event_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Event {} not found", event_id);
            StatusCode::NOT_FOUND
        })?;

    // Check if requesting server has permission to access this event
    let has_permission = check_event_access_permission(&state, &event, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check event access permissions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !has_permission {
        warn!(
            "Server {} not authorized to access event {} in room {}",
            x_matrix_auth.origin, event_id, event.room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Convert Event to PDU format
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

    // Create transaction response
    let transaction = Transaction {
        origin: state.homeserver_name.clone(),
        origin_server_ts: Utc::now().timestamp_millis(),
        pdus: vec![pdu],
        edus: vec![],
    };

    info!("Retrieved event {} for server {}", event_id, x_matrix_auth.origin);

    Ok(Json(transaction))
}

/// Check if a server has permission to access a specific event
async fn check_event_access_permission(
    state: &AppState,
    event: &Event,
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
        .bind(("room_id", event.room_id.clone()))
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
    let world_readable = is_room_world_readable(state, &event.room_id).await?;
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
