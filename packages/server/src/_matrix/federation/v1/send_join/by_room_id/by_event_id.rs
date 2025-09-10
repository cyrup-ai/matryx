use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::federation::pdu_validator::{PduValidator, ValidationResult};
use crate::state::AppState;
use matryx_entity::types::{Event, Membership, MembershipState};
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

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

    // Parse comma-separated key=value pairs
    for param in auth_params.split(',') {
        let param = param.trim();

        if let Some((key_name, value)) = param.split_once('=') {
            match key_name.trim() {
                "origin" => {
                    origin = Some(value.trim().to_string());
                },
                "key" => {
                    // Extract key_id from "ed25519:key_id" format
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

/// PUT /_matrix/federation/v1/send_join/{roomId}/{eventId}
///
/// Submits a signed join event to a resident server for it to accept it into the room's graph.
pub async fn put(
    State(state): State<AppState>,
    Path((room_id, event_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "send_join request - origin: {}, room: {}, event: {}",
        x_matrix_auth.origin, room_id, event_id
    );

    // Validate server signature
    let request_body = serde_json::to_string(&payload).unwrap_or_default();
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "PUT",
            "/send_join",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate the event structure
    let sender = payload.get("sender").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing sender in join event");
        StatusCode::BAD_REQUEST
    })?;

    let state_key = payload.get("state_key").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing state_key in join event");
        StatusCode::BAD_REQUEST
    })?;

    let event_type = payload.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing type in join event");
        StatusCode::BAD_REQUEST
    })?;

    // Validate event structure
    if event_type != "m.room.member" {
        warn!("Invalid event type for join: {}", event_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    if sender != state_key {
        warn!("Sender ({}) must equal state_key ({}) for join event", sender, state_key);
        return Err(StatusCode::BAD_REQUEST);
    }

    let membership = payload
        .get("content")
        .and_then(|c| c.get("membership"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            warn!("Missing membership in join event content");
            StatusCode::BAD_REQUEST
        })?;

    if membership != "join" {
        warn!("Invalid membership for join event: {}", membership);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate that the user belongs to the requesting server
    let user_domain = sender.split(':').nth(1).unwrap_or("");
    if user_domain != x_matrix_auth.origin {
        warn!("User {} doesn't belong to origin server {}", sender, x_matrix_auth.origin);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate that event_id in path matches the event
    let payload_event_id = payload.get("event_id").and_then(|v| v.as_str()).unwrap_or("");

    if payload_event_id != event_id {
        warn!("Event ID mismatch: path ({}) vs payload ({})", event_id, payload_event_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate the room exists and we know about it
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

    // Validate the PDU through the 6-step validation pipeline
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let pdu_validator = PduValidator::new(
        state.session_service.clone(),
        event_repo.clone(),
        room_repo.clone(),
        state.db.clone(),
        state.homeserver_name.clone(),
    );

    // Validate the join event PDU
    let validated_event = match pdu_validator.validate_pdu(&payload, &x_matrix_auth.origin).await {
        Ok(ValidationResult::Valid(event)) => {
            info!("Join event {} validated successfully", event.event_id);
            event
        },
        Ok(ValidationResult::SoftFailed { event, reason }) => {
            warn!("Join event {} soft-failed but accepted: {}", event.event_id, reason);
            event
        },
        Ok(ValidationResult::Rejected { event_id, reason }) => {
            warn!("Join event {} rejected: {}", event_id, reason);
            return Err(StatusCode::FORBIDDEN);
        },
        Err(e) => {
            error!("Join event validation failed: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        },
    };

    // Add our server's signature to the join event
    let signed_event = sign_join_event(&state, validated_event).await.map_err(|e| {
        error!("Failed to sign join event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Store the validated and signed join event
    let stored_event = event_repo.create(&signed_event).await.map_err(|e| {
        error!("Failed to store join event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create or update membership record
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let membership = Membership {
        user_id: sender.to_string(),
        room_id: room_id.clone(),
        membership: MembershipState::Join,
        reason: None,
        invited_by: None,
        updated_at: Some(Utc::now()),
        avatar_url: stored_event
            .content
            .get("avatar_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        display_name: stored_event
            .content
            .get("displayname")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        is_direct: Some(false),
        third_party_invite: None,
        join_authorised_via_users_server: None,
    };

    membership_repo.create(&membership).await.map_err(|e| {
        error!("Failed to create membership record: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get current room state (excluding the join event we just processed)
    let room_state = get_room_state(&state, &room_id, Some(&stored_event.event_id))
        .await
        .map_err(|e| {
            error!("Failed to get room state: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get auth chain for the current room state
    let auth_chain = get_auth_chain(&state, &room_state).await.map_err(|e| {
        error!("Failed to get auth chain: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Build response in the Matrix v1 format (array format)
    let response = json!([
        200,
        {
            "state": room_state,
            "auth_chain": auth_chain
        }
    ]);

    info!("Successfully processed join event {} for user {} in room {}", event_id, sender, room_id);

    Ok(Json(response))
}

/// Add our server's signature to a join event
async fn sign_join_event(
    state: &AppState,
    mut event: Event,
) -> Result<Event, Box<dyn std::error::Error + Send + Sync>> {
    // Get our server's signing key
    let signing_key = state
        .session_service
        .get_server_signing_key(&state.homeserver_name)
        .await
        .map_err(|e| format!("Failed to get server signing key: {}", e))?;

    // Create canonical JSON for signing
    let mut event_for_signing = event.clone();
    event_for_signing.signatures = serde_json::from_value(serde_json::Value::Null).ok();
    event_for_signing.unsigned = None;

    let canonical_json = serde_json::to_string(&event_for_signing)?;

    // Sign the event
    let signature = state
        .session_service
        .sign_json(&canonical_json, &signing_key.key_id)
        .await
        .map_err(|e| format!("Failed to sign event: {}", e))?;

    // Add our signature to the event
    if event.signatures.is_none() {
        event.signatures = serde_json::from_value(json!({})).ok();
    }

    let signatures_value = event.signatures.as_ref().map(|s| serde_json::to_value(s).unwrap_or_default()).unwrap_or_default();
    let mut signatures_map: std::collections::HashMap<String, std::collections::HashMap<String, String>> = serde_json::from_value(signatures_value).unwrap_or_default();

    signatures_map.insert(
        state.homeserver_name.clone(),
        [(format!("ed25519:{}", signing_key.key_id), signature)].into_iter().collect(),
    );

    event.signatures = serde_json::from_value(serde_json::to_value(signatures_map)?).ok();

    Ok(event)
}

/// Get the current state of a room
async fn get_room_state(
    state: &AppState,
    room_id: &str,
    exclude_event_id: Option<&str>,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let mut query = "
        SELECT *
        FROM event
        WHERE room_id = $room_id
        AND state_key IS NOT NULL
        AND (
            SELECT COUNT() 
            FROM event e2 
            WHERE e2.room_id = $room_id 
            AND e2.type = event.type 
            AND e2.state_key = event.state_key 
            AND (e2.depth > event.depth OR (e2.depth = event.depth AND e2.origin_server_ts > event.origin_server_ts))
        ) = 0
        ORDER BY type, state_key
    ".to_string();

    let mut bindings = vec![("room_id", room_id.to_string())];

    if let Some(exclude_id) = exclude_event_id {
        query = format!("{} AND event_id != $exclude_event_id", query);
        bindings.push(("exclude_event_id", exclude_id.to_string()));
    }

    let mut response = state.db.query(&query).bind(("room_id", room_id.to_string()));

    if let Some(exclude_id) = exclude_event_id {
        response = response.bind(("exclude_event_id", exclude_id.to_string()));
    }

    let mut response = response.await?;

    let events: Vec<Event> = response.take(0)?;

    // Convert events to JSON format for response
    let state_events: Vec<Value> = events
        .into_iter()
        .map(|event| serde_json::to_value(event).unwrap_or(json!({})))
        .collect();

    Ok(state_events)
}

/// Get the auth chain for a set of state events
async fn get_auth_chain(
    state: &AppState,
    room_state: &[Value],
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let mut auth_event_ids = std::collections::HashSet::new();

    // Collect all auth_events from the state
    for state_event in room_state {
        if let Some(auth_events) = state_event.get("auth_events").and_then(|v| v.as_array()) {
            for auth_event in auth_events {
                if let Some(auth_event_id) = auth_event.as_str() {
                    auth_event_ids.insert(auth_event_id.to_string());
                }
            }
        }
    }

    if auth_event_ids.is_empty() {
        return Ok(vec![]);
    }

    // Convert HashSet to Vec for query binding
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
