use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;

use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::federation::client::FederationClient;
use crate::federation::pdu_validator::{PduValidator, PduValidatorParams, ValidationResult};
use crate::state::AppState;
use matryx_entity::types::{Event, Membership, MembershipState};
use matryx_surrealdb::repository::{
    EventRepository, FederationRepository, KeyServerRepository, MembershipRepository,
    RoomRepository,
};

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

/// Validate invite_room_state parameter according to Matrix 1.16+ specification
///
/// Performs comprehensive validation of the invite_room_state parameter:
/// 1. Validates invite_room_state is an array
/// 2. Checks for presence of required m.room.create event
/// 3. Validates each event format per room version specification
/// 4. Validates all event signatures
/// 5. Ensures all events belong to the same room as the invite
///
/// Returns M_MISSING_PARAM error if validation fails
async fn validate_invite_room_state(
    invite_room_state: &Value,
    room_id: &str,
    room_version: &str,
    origin_server: &str,
    state: &AppState,
) -> Result<(), (StatusCode, Json<Value>)> {
    // 1. Validate invite_room_state is an array
    let events = invite_room_state.as_array().ok_or_else(|| {
        warn!("invite_room_state must be an array");
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errcode": "M_MISSING_PARAM",
                "error": "invite_room_state must be an array"
            })),
        )
    })?;

    // 2. Check for presence of m.room.create event (required by Matrix 1.16+ spec)
    let has_create = events.iter().any(|e| {
        e.get("type").and_then(|t| t.as_str()) == Some("m.room.create")
    });

    if !has_create {
        warn!("invite_room_state missing required m.room.create event");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errcode": "M_MISSING_PARAM",
                "error": "invite_room_state must contain m.room.create event"
            })),
        ));
    }

    debug!("Found m.room.create event in invite_room_state");

    // Create EventSigningEngine for signature validation
    use crate::federation::event_signing::EventSigningEngine;
    let event_signing_engine = EventSigningEngine::new(
        state.session_service.clone(),
        state.db.clone(),
        state.dns_resolver.clone(),
        state.homeserver_name.clone(),
    )
    .map_err(|e| {
        error!("Failed to create EventSigningEngine: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "errcode": "M_UNKNOWN",
                "error": "Internal server error during validation"
            })),
        )
    })?;

    // 3, 4, 5. Validate each event in invite_room_state
    for (idx, event_value) in events.iter().enumerate() {
        // Validate event has required fields per room version
        validate_event_format(event_value, room_version).map_err(|(status, json_err)| {
            warn!("Event {} in invite_room_state failed format validation", idx);
            (status, json_err)
        })?;

        // Validate event belongs to the same room
        if let Some(event_room_id) = event_value.get("room_id").and_then(|r| r.as_str()) {
            if event_room_id != room_id {
                warn!(
                    "Event {} in invite_room_state belongs to different room: {} vs {}",
                    idx, event_room_id, room_id
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "errcode": "M_MISSING_PARAM",
                        "error": "invite_room_state event belongs to different room"
                    })),
                ));
            }
        }

        // Validate event signatures
        // Parse event to Event struct for signature validation
        let event: Event = serde_json::from_value(event_value.clone()).map_err(|e| {
            warn!("Failed to parse event {} in invite_room_state: {}", idx, e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "errcode": "M_MISSING_PARAM",
                    "error": format!("Invalid event format in invite_room_state at index {}", idx)
                })),
            )
        })?;

        // Validate event cryptography (signatures and hashes)
        let expected_servers = vec![origin_server.to_string()];
        event_signing_engine
            .validate_event_crypto(&event, &expected_servers)
            .await
            .map_err(|e| {
                warn!("Event {} in invite_room_state failed signature validation: {}", idx, e);
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "errcode": "M_MISSING_PARAM",
                        "error": format!("Event signature validation failed in invite_room_state: {}", e)
                    })),
                )
            })?;

        debug!("Event {} in invite_room_state validated successfully", idx);
    }

    info!(
        "Successfully validated invite_room_state with {} events for room {}",
        events.len(),
        room_id
    );

    Ok(())
}

/// Validate event format per room version specification
///
/// Performs basic format validation according to Matrix room version requirements
fn validate_event_format(
    event: &Value,
    room_version: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    // Validate required fields exist
    let event_type = event.get("type").and_then(|t| t.as_str()).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errcode": "M_MISSING_PARAM",
                "error": "Event in invite_room_state missing 'type' field"
            })),
        )
    })?;

    let _sender = event.get("sender").and_then(|s| s.as_str()).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errcode": "M_MISSING_PARAM",
                "error": "Event in invite_room_state missing 'sender' field"
            })),
        )
    })?;

    let _content = event.get("content").ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errcode": "M_MISSING_PARAM",
                "error": "Event in invite_room_state missing 'content' field"
            })),
        )
    })?;

    // Room version specific validation
    match room_version {
        "1" | "2" | "3" => {
            // Room v1-v3: event_id is optional in some contexts
        },
        "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" | _ => {
            // Room v4+: Stricter validation
            let _event_id = event.get("event_id").and_then(|e| e.as_str()).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "errcode": "M_MISSING_PARAM",
                        "error": format!("Event in invite_room_state missing 'event_id' field (required for room version {})", room_version)
                    })),
                )
            })?;
        },
    }

    // State events must have state_key
    if event.get("state_key").is_some() || is_state_event_type(event_type) {
        let _state_key = event.get("state_key").ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "errcode": "M_MISSING_PARAM",
                    "error": format!("State event '{}' in invite_room_state missing 'state_key' field", event_type)
                })),
            )
        })?;
    }

    Ok(())
}

/// Check if an event type is a state event
fn is_state_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "m.room.create"
            | "m.room.member"
            | "m.room.power_levels"
            | "m.room.join_rules"
            | "m.room.history_visibility"
            | "m.room.name"
            | "m.room.topic"
            | "m.room.avatar"
            | "m.room.canonical_alias"
            | "m.room.aliases"
            | "m.room.encryption"
            | "m.room.guest_access"
            | "m.room.server_acl"
            | "m.room.tombstone"
    )
}

/// PUT /_matrix/federation/v2/invite/{roomId}/{eventId}
///
/// Invites a remote user to a room. This is the v2 API with improved request format.
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
        "invite v2 request - origin: {}, room: {}, event: {}",
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
            "/invite",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Extract event from v2 request structure
    let event = payload.get("event").ok_or_else(|| {
        warn!("Missing event in v2 request");
        StatusCode::BAD_REQUEST
    })?;

    let room_version = payload.get("room_version").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing room_version in v2 request");
        StatusCode::BAD_REQUEST
    })?;

    let invite_room_state = payload.get("invite_room_state");

    // Validate the event structure
    let sender = event.get("sender").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing sender in invite event");
        StatusCode::BAD_REQUEST
    })?;

    let state_key = event.get("state_key").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing state_key in invite event");
        StatusCode::BAD_REQUEST
    })?;

    let event_type = event.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing type in invite event");
        StatusCode::BAD_REQUEST
    })?;

    // Validate event structure
    if event_type != "m.room.member" {
        warn!("Invalid event type for invite: {}", event_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    let membership = event
        .get("content")
        .and_then(|c| c.get("membership"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            warn!("Missing membership in invite event content");
            StatusCode::BAD_REQUEST
        })?;

    if membership != "invite" {
        warn!("Invalid membership for invite event: {}", membership);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate that the invited user belongs to our server
    let user_domain = state_key.split(':').nth(1).unwrap_or("");
    if user_domain != state.homeserver_name {
        warn!("User {} doesn't belong to our server {}", state_key, state.homeserver_name);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate that sender belongs to the requesting server
    let sender_domain = sender.split(':').nth(1).unwrap_or("");
    if sender_domain != x_matrix_auth.origin {
        warn!("Sender {} doesn't belong to origin server {}", sender, x_matrix_auth.origin);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate that event_id in path matches the event
    let payload_event_id = event.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
    if payload_event_id != event_id {
        warn!("Event ID mismatch: path ({}) vs payload ({})", event_id, payload_event_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate room version compatibility
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

    if room.room_version != room_version {
        warn!(
            "Room version mismatch: room has {}, request has {}",
            room.room_version, room_version
        );
        return Ok(Json(json!({
            "errcode": "M_INCOMPATIBLE_ROOM_VERSION",
            "error": format!("Room version {} not supported", room_version),
            "room_version": room.room_version
        })));
    }

    // Validate invite_room_state parameter (Matrix 1.16+ spec requirement)
    if let Some(room_state) = invite_room_state {
        if let Err((status, json_error)) = validate_invite_room_state(
            room_state,
            &room_id,
            room_version,
            &x_matrix_auth.origin,
            &state,
        )
        .await
        {
            // M_MISSING_PARAM errors are business logic errors, return as Ok(Json(...))
            // Other status codes are HTTP errors, return as Err(StatusCode)
            return if status == StatusCode::BAD_REQUEST {
                Ok(json_error)
            } else {
                Err(status)
            };
        }
    }

    // Check if user is already in the room
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    if let Ok(Some(existing_membership)) =
        membership_repo.get_by_room_user(&room_id, state_key).await
    {
        match existing_membership.membership {
            MembershipState::Join => {
                warn!("User {} is already joined to room {}", state_key, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "User is already in the room"
                })));
            },
            MembershipState::Ban => {
                warn!("User {} is banned from room {}", state_key, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "User is banned from the room"
                })));
            },
            MembershipState::Invite => {
                warn!("User {} is already invited to room {}", state_key, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "User is already invited to the room"
                })));
            },
            _ => {
                // User has other membership status, can proceed with invite
            },
        }
    }

    // Check sender's authorization to invite users
    let can_invite = check_invite_authorization(&state, &room, sender).await.map_err(|e| {
        error!("Failed to check invite authorization: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !can_invite {
        warn!("User {} not authorized to invite users to room {}", sender, room_id);
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "Sender is not allowed to invite users to this room"
        })));
    }

    // Validate the PDU through the validation pipeline
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let federation_repo = Arc::new(FederationRepository::new(state.db.clone()));
    let key_server_repo = Arc::new(KeyServerRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let federation_client = Arc::new(FederationClient::new(
        state.http_client.clone(),
        state.event_signer.clone(),
        state.homeserver_name.clone(),
        state.config.use_https,
    ));
    let params = PduValidatorParams {
        session_service: state.session_service.clone(),
        event_repo: event_repo.clone(),
        room_repo: room_repo.clone(),
        membership_repo: membership_repo.clone(),
        federation_repo: federation_repo.clone(),
        key_server_repo: key_server_repo.clone(),
        federation_client: federation_client.clone(),
        dns_resolver: state.dns_resolver.clone(),
        db: state.db.clone(),
        homeserver_name: state.homeserver_name.clone(),
    };
    let pdu_validator = PduValidator::new(params).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Validate the invite event PDU
    let validated_event = match pdu_validator.validate_pdu(event, &x_matrix_auth.origin).await {
        Ok(ValidationResult::Valid(event)) => {
            info!("Invite event {} validated successfully", event.event_id);
            event
        },
        Ok(ValidationResult::SoftFailed { event, reason }) => {
            warn!("Invite event {} soft-failed but accepted: {}", event.event_id, reason);
            event
        },
        Ok(ValidationResult::Rejected { event_id, reason }) => {
            warn!("Invite event {} rejected: {}", event_id, reason);
            return Ok(Json(json!({
                "errcode": "M_FORBIDDEN",
                "error": format!("Invite rejected: {}", reason)
            })));
        },
        Err(e) => {
            error!("Invite event validation failed: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        },
    };

    // Add our server's signature to the invite event
    let signed_event = sign_invite_event(&state, validated_event).await.map_err(|e| {
        error!("Failed to sign invite event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Store the validated and signed invite event
    let stored_event = event_repo.create(&signed_event).await.map_err(|e| {
        error!("Failed to store invite event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create membership record for the invited user
    let membership = Membership {
        user_id: state_key.to_string(),
        room_id: room_id.clone(),
        membership: MembershipState::Invite,
        reason: None,
        invited_by: Some(sender.to_string()),
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

    // Build response with signed event and optional room state
    let mut response_event = serde_json::to_value(&stored_event).unwrap_or(json!({}));

    // Add invite_room_state to unsigned section if provided
    if let Some(room_state) = invite_room_state {
        if let Some(unsigned) = response_event.get_mut("unsigned") {
            unsigned["invite_room_state"] = room_state.clone();
        } else {
            response_event["unsigned"] = json!({
                "invite_room_state": room_state
            });
        }
    }

    // Build response in the Matrix v2 format (direct object)
    let response = json!({
        "event": response_event
    });

    info!(
        "Successfully processed invite event {} for user {} in room {} (v2)",
        event_id, state_key, room_id
    );

    Ok(Json(response))
}

/// Check if a user is authorized to invite users to a room
async fn check_invite_authorization(
    state: &AppState,
    room: &matryx_entity::types::Room,
    sender: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Get sender's membership and power level
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let sender_membership = membership_repo.get_by_room_user(&room.room_id, sender).await?;

    // Sender must be in the room to invite others
    match sender_membership {
        Some(membership) if membership.membership == MembershipState::Join => {
            // Check power levels for invite permission
            let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
            room_repo
                .check_invite_power_level(&room.room_id, sender)
                .await
                .map_err(|e| format!("Failed to check invite power level: {}", e).into())
        },
        _ => {
            // Sender is not in the room
            Ok(false)
        },
    }
}

/// Add our server's signature to an invite event
async fn sign_invite_event(
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

    let signatures_value = event
        .signatures
        .as_ref()
        .map(|s| serde_json::to_value(s).unwrap_or_default())
        .unwrap_or_default();
    let mut signatures_map: std::collections::HashMap<
        String,
        std::collections::HashMap<String, String>,
    > = serde_json::from_value(signatures_value).unwrap_or_default();

    signatures_map.insert(
        state.homeserver_name.clone(),
        [(format!("ed25519:{}", signing_key.key_id), signature)]
            .into_iter()
            .collect(),
    );

    event.signatures = serde_json::from_value(serde_json::to_value(signatures_map)?).ok();

    Ok(event)
}
