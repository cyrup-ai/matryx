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
    EventRepository,
    FederationRepository,
    KeyServerRepository,
    MembershipRepository,
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

/// PUT /_matrix/federation/v1/invite/{roomId}/{eventId}
///
/// Invites a remote user to a room. Once the event has been signed by both the inviting
/// homeserver and the invited homeserver, it can be sent to all of the servers in the room.
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
        "invite v1 request - origin: {}, room: {}, event: {}",
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

    // Validate the event structure
    let sender = payload.get("sender").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing sender in invite event");
        StatusCode::BAD_REQUEST
    })?;

    let state_key = payload.get("state_key").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing state_key in invite event");
        StatusCode::BAD_REQUEST
    })?;

    let event_type = payload.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing type in invite event");
        StatusCode::BAD_REQUEST
    })?;

    // Validate event structure
    if event_type != "m.room.member" {
        warn!("Invalid event type for invite: {}", event_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    let membership = payload
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

    // For v1 API, assume room version is 1 or 2 if not specified
    let room_version = room.room_version.clone();
    if !["1", "2"].contains(&room_version.as_str()) {
        warn!("Room version {} not supported by v1 invite API", room_version);
        return Err(StatusCode::BAD_REQUEST);
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
    ));
    let pdu_validator = PduValidator::new(PduValidatorParams {
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
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Validate the invite event PDU
    let validated_event = match pdu_validator.validate_pdu(&payload, &x_matrix_auth.origin).await {
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

    // Build response in the Matrix v1 format (array format)
    let response = json!([
        200,
        {
            "event": serde_json::to_value(&stored_event).unwrap_or(json!({}))
        }
    ]);

    info!(
        "Successfully processed invite event {} for user {} in room {}",
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
            room_repo.check_invite_power_level(&room.room_id, sender).await
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
