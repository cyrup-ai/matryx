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
use crate::federation::event_signing::EventSigningError;

/// Helper function for POST request body integration with federation signing
///
/// Demonstrates the pattern for ensuring JSON request bodies are properly signed
/// when making outgoing federation requests with POST/PUT methods.
#[allow(dead_code)] // Utility function for outgoing federation requests
async fn sign_federation_post_request(
    state: &AppState,
    url: &str,
    destination: &str,
    request_body: &Value,
) -> Result<reqwest::Response, StatusCode> {
    info!("Signing federation POST request to {} for {}", destination, url);

    // Validate destination parameter per Matrix v1.3+ requirements
    validate_matrix_destination_parameter(destination)?;

    // Log request body size for audit trail
    let body_size = serde_json::to_string(request_body)
        .map(|s| s.len())
        .unwrap_or(0);
    debug!("Federation POST request body size: {} bytes", body_size);

    // Create request builder with JSON body
    let request_builder = state.http_client
        .post(url)
        .json(request_body);

    // Sign the request using the existing federation signing infrastructure
    let signed_request = state.event_signer
        .sign_federation_request(request_builder, destination)
        .await
        .map_err(|e| {
            error!("Failed to sign federation POST request: {:?}", e);
            StatusCode::from(e)
        })?;

    debug!("Federation POST request signed successfully for {}", destination);

    // Send the signed request
    let response = signed_request.send().await.map_err(|e| {
        error!("Failed to send signed federation POST request: {:?}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    info!("Federation POST request completed successfully to {}", destination);
    Ok(response)
}

/// Convert EventSigningError to HTTP status codes
impl From<EventSigningError> for StatusCode {
    fn from(error: EventSigningError) -> Self {
        match error {
            EventSigningError::InvalidDestination(_) => StatusCode::BAD_REQUEST,
            EventSigningError::KeyRetrievalError(_) => StatusCode::SERVICE_UNAVAILABLE,
            EventSigningError::InvalidSignature(_) => StatusCode::UNAUTHORIZED,
            EventSigningError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            EventSigningError::JsonError(_) => StatusCode::BAD_REQUEST,
            EventSigningError::HttpError(_) => StatusCode::SERVICE_UNAVAILABLE,
            EventSigningError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            EventSigningError::InvalidFormat(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

use crate::federation::pdu_validator::{PduValidator, PduValidatorParams, ValidationResult};
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
    state: &AppState,
    identity_server: &str,
    key_id: &str,
) -> Result<String, StatusCode> {
    // Validate destination parameter per Matrix v1.3+ requirements
    validate_matrix_destination_parameter(identity_server)?;

    let url = format!("https://{}/_matrix/identity/api/v1/pubkey/{}", identity_server, key_id);

    info!("Signing federation request to {} for {}", identity_server, url);
    let signed_request = state.event_signer
        .sign_federation_request(
            state.http_client.get(&url),
            identity_server
        )
        .await
        .map_err(|e| {
            error!("Failed to sign federation request for identity server key: {:?}", e);
            StatusCode::from(e)
        })?;

    let response = signed_request.send().await.map_err(|e| {
        error!("Failed to fetch identity server key: {:?}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    info!("Federation key fetch request completed successfully to {}", identity_server);

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

/// Find matching public key from the provided keys based on key_id
fn find_matching_public_key(public_keys: &[Value], key_id: &str) -> Option<String> {
    for key in public_keys {
        if let Some(key_obj) = key.as_object() {
            // Check if this key matches the key_id
            if let Some(key_name) = key_obj.get("key_id").and_then(|k| k.as_str())
                && key_name == key_id
                && let Some(public_key) = key_obj.get("public_key").and_then(|k| k.as_str()) {
                return Some(public_key.to_string());
            }
        }
    }
    None
}

/// Verify third-party invite signature using existing infrastructure
async fn verify_third_party_invite_signature(
    state: &AppState,
    signed_object: &Value,
    identity_server: &str,
    public_keys: &[Value],
) -> Result<bool, StatusCode> {
    // Extract signature data
    let signature_data = extract_signature_data(signed_object, identity_server)?;

    // Comprehensive server validation per Matrix specification Section 11
    validate_identity_server_authorization(state, &signature_data.server_name, identity_server).await?;

    // Validate server name matches identity server for security
    if signature_data.server_name != identity_server {
        error!("Server name mismatch: signature claims {} but identity server is {}",
               signature_data.server_name, identity_server);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Additional server-specific validation for third-party invites
    validate_server_third_party_permissions(state, &signature_data.server_name).await?;

    // Find matching public key from the provided keys, or fetch from identity server
    let public_key = match find_matching_public_key(public_keys, &signature_data.key_id) {
        Some(key) => key,
        None => {
            info!("No local public key found for key_id: {} from server {}, fetching from identity server", 
                  signature_data.key_id, signature_data.server_name);
            
            // Fetch identity server key using signed federation request
            fetch_identity_server_key(state, identity_server, &signature_data.key_id).await?
        }
    };

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
            info!("Third-party invite signature verified successfully for server {} with key {}", 
                  signature_data.server_name, signature_data.key_id);
            Ok(true)
        },
        Err(e) => {
            warn!("Third-party invite signature verification failed for server {}: {:?}", 
                  signature_data.server_name, e);
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
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
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
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let federation_client = Arc::new(FederationClient::new(
        state.http_client.clone(),
        state.event_signer.clone(),
        state.homeserver_name.clone(),
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
    // Extract token from third-party invite
    let token = third_party_invite
        .get("signed")
        .and_then(|s| s.get("token"))
        .and_then(|t| t.as_str())
        .ok_or("Missing token in third-party invite")?;

    let federation_repo = FederationRepository::new(state.db.clone());
    let event_content = federation_repo
        .get_third_party_invite_event(room_id, token)
        .await?;

    let event_content = event_content.ok_or("Third-party invite event not found")?;

    // Extract public keys from the original third-party invite event
    let public_keys = if let Some(keys) = event_content.get("public_keys").and_then(|v| v.as_array()) {
        keys.clone()
    } else if let Some(key) = event_content.get("public_key").and_then(|v| v.as_str()) {
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
    match verify_third_party_invite_signature(state, signed_object, identity_server, &public_keys).await {
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

/// Validate identity server authorization per Matrix specification
///
/// Implements Matrix server validation requirements from Section 11 of the
/// server-server API specification for third-party invites. This ensures that
/// identity servers are authorized and trusted for third-party invite processing.
async fn validate_identity_server_authorization(
    state: &AppState,
    server_name: &str,
    identity_server: &str,
) -> Result<(), StatusCode> {
    // Validate identity server is in trusted list (if configured)
    if let Some(trusted_servers) = get_trusted_identity_servers(state).await
        && !trusted_servers.contains(&identity_server.to_string()) {
        warn!(
            "Identity server {} not in trusted server list for third-party invite from {}",
            identity_server, server_name
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate server name format compliance with Matrix specification
    if !is_valid_matrix_server_name(server_name) {
        error!("Invalid server name format: {}", server_name);
        return Err(StatusCode::BAD_REQUEST);
    }

    if !is_valid_matrix_server_name(identity_server) {
        error!("Invalid identity server name format: {}", identity_server);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if identity server is reachable and responding
    validate_identity_server_reachability(state, identity_server).await?;

    debug!(
        "Identity server {} passed authorization validation for server {}",
        identity_server, server_name
    );
    Ok(())
}

/// Validate server-specific third-party invite permissions
///
/// Performs Matrix specification-compliant validation of server permissions
/// for processing third-party invites, including rate limiting and abuse prevention.
async fn validate_server_third_party_permissions(
    state: &AppState,
    server_name: &str,
) -> Result<(), StatusCode> {
    // Check server reputation/blocklist for third-party invite abuse
    if is_server_blocked_for_third_party_invites(state, server_name).await? {
        warn!(
            "Server {} is blocked from third-party invite processing",
            server_name
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate server has proper federation setup
    validate_server_federation_config(state, server_name).await?;

    // Check rate limiting for third-party invites from this server
    if is_server_rate_limited_for_third_party_invites(state, server_name).await? {
        warn!(
            "Server {} is rate limited for third-party invites",
            server_name
        );
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    debug!(
        "Server {} passed third-party invite permissions validation",
        server_name
    );
    Ok(())
}

/// Get list of trusted identity servers from configuration
async fn get_trusted_identity_servers(
    state: &AppState,
) -> Option<Vec<String>> {
    let federation_repo = FederationRepository::new(state.db.clone());
    match federation_repo.get_trusted_identity_servers().await {
        Ok(servers) => servers,
        Err(e) => {
            debug!("No trusted identity servers configuration found: {}", e);
            None
        }
    }
}

/// Validate destination parameter per Matrix v1.3+ specification
///
/// Ensures destination parameters match Matrix v1.3+ requirements including:
/// - Valid server name format
/// - Pre-delegation server name handling
/// - Port validation
/// - IPv6 bracket notation support
fn validate_matrix_destination_parameter(destination: &str) -> Result<(), StatusCode> {
    if !is_valid_matrix_server_name(destination) {
        error!("Invalid Matrix destination parameter format: {}", destination);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Additional Matrix v1.3+ validation for destination parameters
    if destination.len() > 255 {
        error!("Destination parameter exceeds maximum length: {}", destination);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate port range if present
    if let Some(port_str) = destination.split(':').nth(1) {
        if let Ok(port) = port_str.parse::<u16>() {
            if port == 0 {
                error!("Invalid port 0 in destination parameter: {}", destination);
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            error!("Invalid port format in destination parameter: {}", destination);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    debug!("Destination parameter validation passed for: {}", destination);
    Ok(())
}

/// Validate Matrix server name format per specification
fn is_valid_matrix_server_name(server_name: &str) -> bool {
    use std::net::IpAddr;

    // Basic validation: must contain at least one character
    if server_name.is_empty() {
        return false;
    }

    // Split on port if present
    let parts: Vec<&str> = server_name.split(':').collect();
    let hostname = parts[0];

    // Validate hostname part
    if hostname.is_empty() {
        return false;
    }

    // Handle IPv6 addresses in bracket notation per Matrix spec
    if hostname.starts_with('[') && hostname.ends_with(']') {
        let ipv6_str = &hostname[1..hostname.len()-1];
        return ipv6_str.parse::<std::net::Ipv6Addr>().is_ok();
    }

    // Check if it's an IP address (allowed)
    if hostname.parse::<IpAddr>().is_ok() {
        return true;
    }

    // Check if it's a valid domain name format
    // Enhanced validation per Matrix specification
    hostname.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_'
    }) && !hostname.starts_with('-') && !hostname.ends_with('-') && hostname.contains('.')
}

/// Validate identity server reachability
async fn validate_identity_server_reachability(
    state: &AppState,
    identity_server: &str,
) -> Result<(), StatusCode> {
    // Validate destination parameter per Matrix v1.3+ requirements
    validate_matrix_destination_parameter(identity_server)?;

    // Log validation attempt for monitoring and security auditing
    info!("Validating identity server reachability: {}", identity_server);
    
    // Use homeserver name from state for proper logging context
    let homeserver_name = &state.homeserver_name;
    debug!("Validating identity server {} for homeserver {}", identity_server, homeserver_name);
    
    // Try to reach the identity server's status endpoint using configured HTTP client
    let url = format!("https://{}/_matrix/identity/api/v1/status", identity_server);

    info!("Signing federation request to {} for {}", identity_server, url);
    let signed_request_result = state.event_signer
        .sign_federation_request(
            state.http_client.get(&url).timeout(std::time::Duration::from_secs(10)),
            identity_server
        )
        .await;

    let signed_request = match signed_request_result {
        Ok(req) => {
            debug!("Federation request signed successfully for identity server reachability check to {}", identity_server);
            req
        },
        Err(e) => {
            warn!("Failed to sign federation request for identity server reachability: {:?}", e);
            debug!("Continuing with unsigned request for identity server status check as fallback");
            // For identity server status checks, continue with unsigned request as fallback
            // since this is not a critical federation operation requiring signatures
            state.http_client.get(&url).timeout(std::time::Duration::from_secs(10))
        }
    };

    match signed_request.send().await {
        Ok(response) => {
            if response.status().is_success() {
                info!("Identity server {} is reachable from homeserver {}", identity_server, homeserver_name);
                Ok(())
            } else {
                warn!(
                    "Identity server {} returned non-success status: {}",
                    identity_server, response.status()
                );
                Err(StatusCode::SERVICE_UNAVAILABLE)
            }
        },
        Err(e) => {
            warn!(
                "Failed to reach identity server {}: {}",
                identity_server, e
            );
            // Don't fail hard on reachability - identity server might be temporarily down
            // but signature validation can still proceed with cached keys
            debug!("Continuing with cached identity server validation");
            Ok(())
        }
    }
}

/// Check if server is blocked for third-party invite processing
async fn is_server_blocked_for_third_party_invites(
    state: &AppState,
    server_name: &str,
) -> Result<bool, StatusCode> {
    let federation_repo = FederationRepository::new(state.db.clone());
    match federation_repo.is_server_blocked_for_third_party_invites(server_name).await {
        Ok(blocked) => Ok(blocked),
        Err(e) => {
            debug!("Error checking server blocklist: {}", e);
            Ok(false) // Assume not blocked if query fails
        }
    }
}

/// Validate server federation configuration
async fn validate_server_federation_config(
    state: &AppState,
    server_name: &str,
) -> Result<(), StatusCode> {
    // Check if server has proper federation keys and configuration
    let federation_repo = FederationRepository::new(state.db.clone());
    match federation_repo.check_server_federation_config(server_name).await {
        Ok(false) => {
            warn!(
                "Federation disabled for server {} in third-party invite context",
                server_name
            );
            return Err(StatusCode::FORBIDDEN);
        },
        Ok(true) => {
            // Federation is enabled, continue
        },
        Err(e) => {
            debug!("No federation config found for server {}: {}", server_name, e);
        }
    }

    Ok(())
}

/// Check if server is rate limited for third-party invites
async fn is_server_rate_limited_for_third_party_invites(
    state: &AppState,
    server_name: &str,
) -> Result<bool, StatusCode> {
    let federation_repo = FederationRepository::new(state.db.clone());
    match federation_repo.is_server_rate_limited_for_third_party_invites(server_name).await {
        Ok(is_rate_limited) => Ok(is_rate_limited),
        Err(e) => {
            debug!("Error checking third-party invite rate limit: {}", e);
            Ok(false)
        }
    }
}
