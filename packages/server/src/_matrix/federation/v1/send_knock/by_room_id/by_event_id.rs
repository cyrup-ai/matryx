use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::federation::membership_federation::validate_room_knock_allowed;
use crate::state::AppState;
use matryx_entity::types::MembershipState;
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

/// PUT /_matrix/federation/v1/send_knock/{roomId}/{eventId}
///
/// Submits a signed knock event to the resident server for it to accept into the room's graph.
/// The resident server must accept the knock if the event is valid and the server is allowed to knock.
pub async fn put(
    State(state): State<AppState>,
    Path((room_id, event_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
    })?;

    debug!(
        "send_knock request - origin: {}, room: {}, event: {}",
        x_matrix_auth.origin, room_id, event_id
    );

    // Validate server signature
    let request_uri = format!("/send_knock/{}/{}", room_id, event_id);
    let request_body = serde_json::to_vec(&payload).map_err(|_| StatusCode::BAD_REQUEST)?;

    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "PUT",
            &request_uri,
            &request_body,
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

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

    // Validate room allows knocking per Matrix specification
    if !validate_room_knock_allowed(&room, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to validate knock permissions for room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        warn!(
            "Knock denied for server {} in room {} - room join rules don't permit knocking",
            x_matrix_auth.origin, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Check room version compatibility for knock support
    let version_num = room.room_version.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(1);
    if version_num < 7 {
        warn!(
            "Knock not supported in room {} with version {} - knocking requires room version 7+",
            room_id, room.room_version
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    info!(
        "Room knock validation passed for room {} (version {}) from server {}",
        room_id, room.room_version, x_matrix_auth.origin
    );

    // Extract and validate the knock event from payload
    let knock_event = payload.as_object().ok_or_else(|| {
        warn!("Invalid payload format - expected JSON object");
        StatusCode::BAD_REQUEST
    })?;

    // Validate basic event structure
    let event_type = knock_event.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing or invalid event type");
        StatusCode::BAD_REQUEST
    })?;

    if event_type != "m.room.member" {
        warn!("Invalid event type for knock: {}", event_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    let event_room_id = knock_event.get("room_id").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing or invalid room_id in event");
        StatusCode::BAD_REQUEST
    })?;

    if event_room_id != room_id {
        warn!("Event room_id {} doesn't match path room_id {}", event_room_id, room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    let event_event_id = knock_event.get("event_id").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing or invalid event_id in event");
        StatusCode::BAD_REQUEST
    })?;

    if event_event_id != event_id {
        warn!("Event event_id {} doesn't match path event_id {}", event_event_id, event_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    let sender = knock_event.get("sender").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing or invalid sender in event");
        StatusCode::BAD_REQUEST
    })?;

    let state_key = knock_event.get("state_key").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing or invalid state_key in event");
        StatusCode::BAD_REQUEST
    })?;

    if sender != state_key {
        warn!("Sender {} doesn't match state_key {} for membership event", sender, state_key);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate that the sender belongs to the requesting server
    let user_domain = sender.split(':').nth(1).unwrap_or("");
    if user_domain != x_matrix_auth.origin {
        warn!("Sender {} doesn't belong to origin server {}", sender, x_matrix_auth.origin);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate membership content
    let content = knock_event.get("content").and_then(|v| v.as_object()).ok_or_else(|| {
        warn!("Missing or invalid content in event");
        StatusCode::BAD_REQUEST
    })?;

    let membership = content.get("membership").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing or invalid membership in event content");
        StatusCode::BAD_REQUEST
    })?;

    if membership != "knock" {
        warn!("Invalid membership type for knock event: {}", membership);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if room allows knocking
    let join_rules_valid = check_room_allows_knocking(&state, &room_id).await.map_err(|e| {
        error!("Failed to check room join rules: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !join_rules_valid {
        warn!("Room {} does not allow knocking", room_id);
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "You are not permitted to knock on this room"
        })));
    }

    // Check if user is already in the room or banned
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    if let Ok(Some(existing_membership)) = membership_repo.get_by_room_user(&room_id, sender).await
    {
        match existing_membership.membership {
            MembershipState::Join => {
                warn!("User {} is already joined to room {}", sender, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are already in the room"
                })));
            },
            MembershipState::Ban => {
                warn!("User {} is banned from room {}", sender, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are banned from the room"
                })));
            },
            MembershipState::Knock => {
                warn!("User {} is already knocking on room {}", sender, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are already knocking on this room"
                })));
            },
            MembershipState::Invite => {
                warn!("User {} is already invited to room {}", sender, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are already invited to this room"
                })));
            },
            _ => {
                // User has other membership status (e.g., left), can proceed with knock
            },
        }
    }

    // Check server ACLs
    let server_allowed =
        check_server_acls(&state, &room_id, &x_matrix_auth.origin)
            .await
            .map_err(|e| {
                error!("Failed to check server ACLs: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    if !server_allowed {
        warn!("Server {} is denied by room ACLs", x_matrix_auth.origin);
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "Your server is not permitted to knock on this room"
        })));
    }

    // Validate event signatures
    let signatures_valid = validate_event_signatures(&state, &payload).await.map_err(|e| {
        error!("Failed to validate event signatures: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !signatures_valid {
        warn!("Event signatures validation failed for event {}", event_id);
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "Invalid event signature"
        })));
    }

    // Run PDU validation pipeline
    let pdu_valid = validate_pdu_structure(&payload).map_err(|e| {
        warn!("PDU validation failed: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    if !pdu_valid {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check authorization rules for the knock event
    let auth_valid = check_knock_authorization(&state, &room_id, sender, &payload)
        .await
        .map_err(|e| {
            error!("Failed to check knock authorization: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !auth_valid {
        warn!("Authorization check failed for knock event from {}", sender);
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "You are not authorized to knock on this room"
        })));
    }

    // Store the knock event
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));

    // Convert payload to Event entity for storage
    let event_entity = matryx_entity::Event {
        event_id: event_id.clone(),
        room_id: room_id.clone(),
        sender: sender.to_string(),
        event_type: "m.room.member".to_string(),
        origin_server_ts: knock_event
            .get("origin_server_ts")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| Utc::now().timestamp_millis()),
        content: matryx_entity::EventContent::unknown(
            payload.get("content").cloned().unwrap_or_default(),
        ),
        state_key: Some(sender.to_string()),
        unsigned: knock_event.get("unsigned").cloned(),
        prev_events: knock_event
            .get("prev_events")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()),
        auth_events: knock_event
            .get("auth_events")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()),
        depth: knock_event.get("depth").and_then(|v| v.as_i64()),
        hashes: knock_event.get("hashes").and_then(|v| v.as_object()).map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        }),
        signatures: knock_event.get("signatures").and_then(|v| v.as_object()).map(|obj| {
            obj.iter()
                .map(|(server, sigs)| {
                    let sig_map = sigs
                        .as_object()
                        .map(|s| {
                            s.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    (server.clone(), sig_map)
                })
                .collect()
        }),
        redacts: knock_event.get("redacts").and_then(|v| v.as_str()).map(|s| s.to_string()),
        soft_failed: None,
        received_ts: Some(Utc::now().timestamp_millis()),
        outlier: Some(false),
        rejected_reason: None,
    };

    event_repo.create(&event_entity).await.map_err(|e| {
        error!("Failed to store knock event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Update membership state
    let membership_data = matryx_entity::Membership {
        room_id: room_id.clone(),
        user_id: sender.to_string(),
        membership: MembershipState::Knock,
        reason: content.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
        avatar_url: content.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
        display_name: content.get("displayname").and_then(|v| v.as_str()).map(|s| s.to_string()),
        is_direct: content.get("is_direct").and_then(|v| v.as_bool()),
        join_authorised_via_users_server: content
            .get("join_authorised_via_users_server")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        third_party_invite: None,
        invited_by: None,
        updated_at: Some(Utc::now()),
    };

    membership_repo.create(&membership_data).await.map_err(|e| {
        error!("Failed to create membership record: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get room state events to return with the knock response
    let knock_state_events = get_room_state_for_knock(&state, &room_id).await.map_err(|e| {
        error!("Failed to get room state for knock: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = json!({
        "knock_state_events": knock_state_events
    });

    info!("Successfully processed knock from user {} for room {}", sender, room_id);

    Ok(Json(response))
}

/// Check if room allows knocking by examining join rules using repository
async fn check_room_allows_knocking(
    state: &AppState,
    room_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let allows_knocking = room_repo
        .check_room_allows_knocking(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(allows_knocking)
}

/// Check if server is allowed by room ACLs using repository
async fn check_server_acls(
    state: &AppState,
    room_id: &str,
    server_name: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let server_allowed = room_repo
        .check_server_acls(room_id, server_name)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(server_allowed)
}

/// Validate event signatures
async fn validate_event_signatures(
    state: &AppState,
    event: &Value,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Extract signatures from event
    let signatures = event
        .get("signatures")
        .and_then(|s| s.as_object())
        .ok_or("Missing signatures in event")?;

    // Extract origin server from event
    let origin = event
        .get("origin")
        .and_then(|o| o.as_str())
        .ok_or("Missing origin in event")?;

    // Get server signatures for the origin
    let server_sigs = signatures
        .get(origin)
        .and_then(|s| s.as_object())
        .ok_or("Missing server signatures for origin")?;

    // Validate each signature
    for (key_id, signature) in server_sigs {
        if !key_id.starts_with("ed25519:") {
            continue; // Skip non-ed25519 signatures
        }

        let sig_str = signature.as_str().ok_or("Invalid signature format")?;

        // Create canonical JSON for signature verification
        let mut event_copy = event.clone();
        if let Some(obj) = event_copy.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        let canonical_json = serde_json::to_string(&event_copy)?;

        // Validate signature using session service
        let key_id_only = key_id.strip_prefix("ed25519:").unwrap_or(key_id);
        let validation_result = state
            .session_service
            .validate_server_signature(
                origin,
                key_id_only,
                sig_str,
                "POST",    // Method doesn't matter for event signatures
                "/events", // Path doesn't matter for event signatures
                canonical_json.as_bytes(),
            )
            .await;

        if validation_result.is_err() {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Validate PDU structure
fn validate_pdu_structure(event: &Value) -> Result<bool, String> {
    let event_obj = event.as_object().ok_or("Event must be a JSON object")?;

    // Required fields for PDU
    let required_fields = [
        "type",
        "content",
        "event_id",
        "sender",
        "origin_server_ts",
        "room_id",
        "signatures",
    ];

    for field in &required_fields {
        if !event_obj.contains_key(*field) {
            return Err(format!("Missing required field: {}", field));
        }
    }

    // Validate event_id format
    let event_id = event_obj
        .get("event_id")
        .and_then(|v| v.as_str())
        .ok_or("Invalid event_id")?;

    if !event_id.starts_with('$') {
        return Err("event_id must start with $".to_string());
    }

    // Validate sender format
    let sender = event_obj.get("sender").and_then(|v| v.as_str()).ok_or("Invalid sender")?;

    if !sender.starts_with('@') {
        return Err("sender must start with @".to_string());
    }

    // Validate room_id format
    let room_id = event_obj
        .get("room_id")
        .and_then(|v| v.as_str())
        .ok_or("Invalid room_id")?;

    if !room_id.starts_with('!') {
        return Err("room_id must start with !".to_string());
    }

    // Validate origin_server_ts is a number
    event_obj
        .get("origin_server_ts")
        .and_then(|v| v.as_i64())
        .ok_or("origin_server_ts must be a number")?;

    Ok(true)
}

/// Check authorization rules for knock event using repository
async fn check_knock_authorization(
    state: &AppState,
    room_id: &str,
    user_id: &str,
    _event: &Value,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let authorized = event_repo
        .check_knock_authorization(room_id, user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(authorized)
}

/// Get room state events to include in knock response using repository
async fn get_room_state_for_knock(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let state_events = event_repo
        .get_room_state_for_knock(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(state_events)
}
