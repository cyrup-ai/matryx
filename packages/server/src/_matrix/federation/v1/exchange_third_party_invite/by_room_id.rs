use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::federation::pdu_validator::{PduValidator, ValidationResult};
use crate::state::AppState;
use crate::utils::canonical_json::to_canonical_json;
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

/// Signature data extracted from third-party invite
#[derive(Debug, Clone)]
struct SignatureData {
    server_name: String,
    key_id: String,
    signature: String,
}

/// Extract signature data from third-party invite
fn extract_signature_data(
    signed_object: &Value,
    identity_server: &str,
) -> Result<SignatureData, StatusCode> {
    let signatures = signed_object
        .get("signatures")
        .and_then(|s| s.as_object())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let server_signatures = signatures
        .get(identity_server)
        .and_then(|s| s.as_object())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Find Ed25519 signature
    let (key_id, signature_b64) = server_signatures
        .iter()
        .find(|(k, _)| k.starts_with("ed25519:"))
        .ok_or(StatusCode::BAD_REQUEST)?;

    let signature_str = signature_b64.as_str().ok_or(StatusCode::BAD_REQUEST)?;

    Ok(SignatureData {
        server_name: identity_server.to_string(),
        key_id: key_id.clone(),
        signature: signature_str.to_string(),
    })
}

/// Fetch identity server public key
async fn fetch_identity_server_key(
    http_client: &Client,
    identity_server: &str,
    key_id: &str,
) -> Result<String, StatusCode> {
    let url = format!("https://{}/_matrix/identity/api/v1/pubkey/{}", identity_server, key_id);

    let response = http_client.get(&url).send().await.map_err(|e| {
        error!("Failed to fetch identity server key: {:?}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    if !response.status().is_success() {
        error!("Identity server returned error: {}", response.status());
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let key_data: Value = response.json().await.map_err(|e| {
        error!("Failed to parse identity server key response: {:?}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let public_key = key_data.get("public_key").and_then(|k| k.as_str()).ok_or_else(|| {
        error!("Invalid public key format in identity server response");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(public_key.to_string())
}

/// Verify third-party invite signature using existing infrastructure
async fn verify_third_party_invite_signature(
    state: &AppState,
    signed_object: &Value,
    identity_server: &str,
) -> Result<bool, StatusCode> {
    // Extract signature data
    let signature_data = extract_signature_data(signed_object, identity_server)?;

    // Fetch public key from identity server
    let public_key = fetch_identity_server_key(
        &state.http_client,
        &signature_data.server_name,
        &signature_data.key_id,
    )
    .await?;

    // Create canonical JSON without signatures
    let mut canonical_object = signed_object.clone();
    if let Some(obj) = canonical_object.as_object_mut() {
        obj.remove("signatures");
    }

    let canonical_json = to_canonical_json(&canonical_object).map_err(|e| {
        error!("Failed to create canonical JSON: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Use existing Ed25519 verification from MatrixSessionService
    match state.session_service.verify_ed25519_signature(
        &signature_data.signature,
        &canonical_json,
        &public_key,
    ) {
        Ok(_) => {
            debug!("Third-party invite signature verified successfully");
            Ok(true)
        },
        Err(e) => {
            warn!("Third-party invite signature verification failed: {:?}", e);
            Ok(false)
        },
    }
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

/// PUT /_matrix/federation/v1/exchange_third_party_invite/{roomId}
///
/// Exchanges a third-party invite for a standard Matrix invite. The receiving server
/// will verify the partial m.room.member event and issue an invite if valid.
pub async fn put(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "exchange_third_party_invite request - origin: {}, room: {}",
        x_matrix_auth.origin, room_id
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
            "/exchange_third_party_invite",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Extract and validate request structure
    let payload_room_id = payload.get("room_id").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing room_id in request");
        StatusCode::BAD_REQUEST
    })?;

    let event_type = payload.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing type in request");
        StatusCode::BAD_REQUEST
    })?;

    let sender = payload.get("sender").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing sender in request");
        StatusCode::BAD_REQUEST
    })?;

    let state_key = payload.get("state_key").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing state_key in request");
        StatusCode::BAD_REQUEST
    })?;

    let content = payload.get("content").ok_or_else(|| {
        warn!("Missing content in request");
        StatusCode::BAD_REQUEST
    })?;

    let membership = content.get("membership").and_then(|v| v.as_str()).ok_or_else(|| {
        warn!("Missing membership in content");
        StatusCode::BAD_REQUEST
    })?;

    let third_party_invite = content.get("third_party_invite").ok_or_else(|| {
        warn!("Missing third_party_invite in content");
        StatusCode::BAD_REQUEST
    })?;

    // Validate request structure
    if payload_room_id != room_id {
        warn!("Room ID mismatch: path ({}) vs payload ({})", room_id, payload_room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    if event_type != "m.room.member" {
        warn!("Invalid event type for third-party invite exchange: {}", event_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    if membership != "invite" {
        warn!("Invalid membership for third-party invite exchange: {}", membership);
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

    // Verify the third-party invite signature
    let signature_valid =
        verify_room_third_party_invite_signature(&state, &room_id, third_party_invite)
            .await
            .map_err(|e| {
                error!("Failed to verify third-party invite signature: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    if !signature_valid {
        warn!("Third-party invite signature verification failed");
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "Invalid third-party invite signature"
        })));
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

    // Create the invite event
    let invite_event = create_invite_event_from_third_party(
        &state,
        &room_id,
        sender,
        state_key,
        third_party_invite,
    )
    .await
    .map_err(|e| {
        error!("Failed to create invite event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Validate the invite event PDU
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let federation_repo = Arc::new(FederationRepository::new(state.db.clone()));
    let key_server_repo = Arc::new(KeyServerRepository::new(state.db.clone()));
    let pdu_validator = PduValidator::new(
        state.session_service.clone(),
        event_repo.clone(),
        room_repo.clone(),
        federation_repo.clone(),
        key_server_repo.clone(),
        state.db.clone(),
        state.homeserver_name.clone(),
    );

    let invite_event_value = serde_json::to_value(&invite_event).map_err(|e| {
        error!("Failed to serialize invite event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let validated_event = match pdu_validator
        .validate_pdu(&invite_event_value, &x_matrix_auth.origin)
        .await
    {
        Ok(ValidationResult::Valid(event)) => {
            info!("Third-party invite event {} validated successfully", event.event_id);
            event
        },
        Ok(ValidationResult::SoftFailed { event, reason }) => {
            warn!(
                "Third-party invite event {} soft-failed but accepted: {}",
                event.event_id, reason
            );
            event
        },
        Ok(ValidationResult::Rejected { event_id, reason }) => {
            warn!("Third-party invite event {} rejected: {}", event_id, reason);
            return Ok(Json(json!({
                "errcode": "M_FORBIDDEN",
                "error": format!("Third-party invite rejected: {}", reason)
            })));
        },
        Err(e) => {
            error!("Third-party invite event validation failed: {}", e);
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
    let display_name = third_party_invite
        .get("display_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

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
        display_name,
        is_direct: Some(false),
        third_party_invite: Some(third_party_invite.clone()),
        join_authorised_via_users_server: None,
    };

    membership_repo.create(&membership).await.map_err(|e| {
        error!("Failed to create membership record: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully processed third-party invite exchange for user {} in room {}",
        state_key, room_id
    );

    // Return empty response as per Matrix spec
    Ok(Json(json!({})))
}

/// Verify the third-party invite signature against the stored room event
async fn verify_room_third_party_invite_signature(
    state: &AppState,
    room_id: &str,
    third_party_invite: &Value,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Find the m.room.third_party_invite event with the matching token
    let query = "
        SELECT content
        FROM event 
        WHERE room_id = $room_id 
        AND type = 'm.room.third_party_invite' 
        AND state_key = $token
        ORDER BY depth DESC, origin_server_ts DESC
        LIMIT 1
    ";

    // Extract token from third-party invite
    let token = third_party_invite
        .get("signed")
        .and_then(|s| s.get("token"))
        .and_then(|t| t.as_str())
        .ok_or("Missing token in third-party invite")?;

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("token", token.to_string()))
        .await?;

    #[derive(serde::Deserialize)]
    struct ThirdPartyInviteEventContent {
        public_keys: Option<Vec<Value>>,
        public_key: Option<String>,
    }

    let event_content: Option<ThirdPartyInviteEventContent> = response.take(0)?;

    let event_content = event_content.ok_or("Third-party invite event not found")?;

    // Extract public keys from the original third-party invite event
    let public_keys = if let Some(keys) = event_content.public_keys {
        keys
    } else if let Some(key) = event_content.public_key {
        vec![json!({ "public_key": key })]
    } else {
        return Err("No public keys found in third-party invite event".into());
    };

    // Get the signed object from third-party invite
    let signed_object = third_party_invite
        .get("signed")
        .ok_or("Missing signed object in third-party invite")?;

    let mxid = signed_object.get("mxid").and_then(|m| m.as_str()).unwrap_or("");

    // Validate that the mxid and token are present
    if mxid.is_empty() || token.is_empty() {
        return Ok(false);
    }

    // Extract identity server from the signatures
    let signatures = signed_object
        .get("signatures")
        .and_then(|s| s.as_object())
        .ok_or("Missing signatures in third-party invite")?;

    // Find the first identity server with Ed25519 signatures
    let identity_server = signatures
        .keys()
        .find(|server| {
            signatures
                .get(*server)
                .and_then(|sigs| sigs.as_object())
                .map(|sigs| sigs.keys().any(|k| k.starts_with("ed25519:")))
                .unwrap_or(false)
        })
        .ok_or("No Ed25519 signatures found")?;

    // Verify the cryptographic signature
    match verify_third_party_invite_signature(state, signed_object, identity_server).await {
        Ok(true) => {
            info!("Third-party invite signature verified successfully");
            Ok(true)
        },
        Ok(false) => {
            warn!("Third-party invite signature verification failed");
            Ok(false)
        },
        Err(StatusCode::BAD_REQUEST) => {
            warn!("Invalid third-party invite signature format");
            Ok(false)
        },
        Err(StatusCode::SERVICE_UNAVAILABLE) => {
            error!("Identity server unavailable for key fetching");
            Ok(false)
        },
        Err(_) => {
            error!("Unexpected error during signature verification");
            Ok(false)
        },
    }
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
            check_invite_power_level(state, &room.room_id, sender).await
        },
        _ => {
            // Sender is not in the room
            Ok(false)
        },
    }
}

/// Check if user has sufficient power level to invite users
async fn check_invite_power_level(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT content.invite, content.users
        FROM event 
        WHERE room_id = $room_id 
        AND type = 'm.room.power_levels' 
        AND state_key = ''
        ORDER BY depth DESC, origin_server_ts DESC
        LIMIT 1
    ";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct PowerLevelsContent {
        invite: Option<i64>,
        users: Option<std::collections::HashMap<String, i64>>,
    }

    let power_levels: Option<PowerLevelsContent> = response.take(0)?;

    match power_levels {
        Some(pl) => {
            let required_level = pl.invite.unwrap_or(0); // Default invite level is 0
            let user_level = pl.users.and_then(|users| users.get(user_id).copied()).unwrap_or(0); // Default user level is 0

            Ok(user_level >= required_level)
        },
        None => {
            // No power levels event, default behavior allows invites
            Ok(true)
        },
    }
}

/// Create an invite event from a third-party invite exchange request
async fn create_invite_event_from_third_party(
    state: &AppState,
    room_id: &str,
    sender: &str,
    state_key: &str,
    third_party_invite: &Value,
) -> Result<Event, Box<dyn std::error::Error + Send + Sync>> {
    // Generate a new event ID
    let event_id = format!("${}:{}", uuid::Uuid::new_v4(), state.homeserver_name);

    // Get current timestamp
    let now = Utc::now();
    let origin_server_ts = now.timestamp_millis() as u64;

    // Extract display name from third-party invite
    let display_name = third_party_invite
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Create event content with third-party invite information
    let content = json!({
        "membership": "invite",
        "displayname": display_name,
        "third_party_invite": third_party_invite
    });

    // Create the invite event
    let event = Event {
        event_id,
        sender: sender.to_string(),
        origin_server_ts: origin_server_ts as i64,
        event_type: "m.room.member".to_string(),
        room_id: room_id.to_string(),
        content: serde_json::from_value(content)?,
        state_key: Some(state_key.to_string()),
        unsigned: None,
        auth_events: Some(vec![]), // Will be populated by PDU validator
        depth: Some(0),            // Will be set by PDU validator
        hashes: Some(std::collections::HashMap::new()),
        prev_events: Some(vec![]), // Will be populated by PDU validator
        signatures: None,          // Will be added when signing
        soft_failed: None,
        received_ts: None,
        outlier: None,
        redacts: None,
        rejected_reason: None,
    };

    Ok(event)
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
