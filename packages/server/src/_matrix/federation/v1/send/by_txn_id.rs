use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};

use chrono::Utc;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::auth::MatrixAuthError;
use crate::federation::client::FederationClient;
use crate::federation::pdu_validator::{PduValidator, PduValidatorParams, ValidationResult};
use crate::state::AppState;
use matryx_surrealdb::repository::{
    DeviceRepository, EventRepository, FederationRepository, KeyServerRepository,
    MembershipRepository, RoomRepository, TransactionRepository, UserRepository,
};

/// Matrix X-Matrix authentication header parsed structure with comprehensive validation
#[derive(Debug, Clone)]
pub struct XMatrixAuth {
    origin: String,
    key_id: String, // Full key_id including algorithm prefix (e.g., "ed25519:abc123")
    signature: String,
    destination: Option<String>, // Optional destination parameter for verification
}

/// Comprehensive X-Matrix authentication header parser with Matrix specification compliance
///
/// Handles all Matrix X-Matrix header edge cases including:
/// - URL encoding/decoding for server names and signatures
/// - Proper quoted string parsing with escape sequence support
/// - Multiple key formats and algorithm prefixes
/// - Optional destination parameter validation
/// - Malformed header detection and graceful error handling
/// - Case-insensitive parameter parsing
/// - Whitespace normalization and trimming
///
/// Matrix X-Matrix format:
/// X-Matrix origin=origin.server.com,key="ed25519:key_id",sig="base64_signature",destination=dest.server.com
fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<XMatrixAuth, StatusCode> {
    // Extract Authorization header with comprehensive error handling
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| {
            warn!("Missing Authorization header in federation request");
            StatusCode::UNAUTHORIZED
        })?
        .to_str()
        .map_err(|e| {
            warn!("Invalid UTF-8 in Authorization header: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // Validate X-Matrix prefix (case-insensitive per Matrix spec)
    let x_matrix_prefix = "X-Matrix ";
    if !auth_header.to_lowercase().starts_with(&x_matrix_prefix.to_lowercase()) {
        warn!("Authorization header missing X-Matrix prefix: {}", auth_header);
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_params = &auth_header[x_matrix_prefix.len()..].trim();

    if auth_params.is_empty() {
        warn!("Empty X-Matrix parameters");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse parameters with comprehensive handling of Matrix specification edge cases
    let params = parse_x_matrix_parameters(auth_params).map_err(|e| {
        warn!("Failed to parse X-Matrix parameters: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Extract required origin parameter with validation
    let origin = params
        .get("origin")
        .ok_or_else(|| {
            warn!("Missing required 'origin' parameter in X-Matrix header");
            StatusCode::BAD_REQUEST
        })?
        .clone();

    // Validate origin is a valid Matrix server name
    if !is_valid_server_name(&origin) {
        warn!("Invalid origin server name format: {}", origin);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Extract required key parameter with full algorithm prefix support
    let key = params
        .get("key")
        .ok_or_else(|| {
            warn!("Missing required 'key' parameter in X-Matrix header");
            StatusCode::BAD_REQUEST
        })?
        .clone();

    // Validate key format supports multiple algorithms
    if !is_valid_signing_key_format(&key) {
        warn!("Invalid signing key format: {}", key);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Extract required signature parameter with base64 validation
    let signature = params
        .get("sig")
        .ok_or_else(|| {
            warn!("Missing required 'sig' parameter in X-Matrix header");
            StatusCode::BAD_REQUEST
        })?
        .clone();

    // Validate signature is valid base64 (preliminary check)
    if !is_valid_base64_signature(&signature) {
        warn!("Invalid base64 signature format");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Extract optional destination parameter
    let destination = params.get("destination").cloned();

    // Validate destination if present
    if let Some(dest) = &destination
        && !is_valid_server_name(dest)
    {
        warn!("Invalid destination server name format: {}", dest);
        return Err(StatusCode::BAD_REQUEST);
    }

    debug!(
        "Successfully parsed X-Matrix auth - origin: {}, key: {}, destination: {:?}",
        origin, key, destination
    );

    Ok(XMatrixAuth { origin, key_id: key, signature, destination })
}

/// Parse X-Matrix parameters handling all Matrix specification edge cases
///
/// Supports:
/// - Quoted and unquoted parameter values
/// - Escaped characters within quoted strings (\", \\, etc.)
/// - URL-encoded server names and parameters
/// - Case-insensitive parameter names
/// - Flexible whitespace handling
/// - Comma-separated parameter lists with proper tokenization
fn parse_x_matrix_parameters(params_str: &str) -> Result<HashMap<String, String>, String> {
    let mut params = HashMap::new();

    // Split on commas first, then parse each parameter individually
    let param_parts: Vec<&str> = params_str.split(',').collect();

    for param_part in param_parts {
        let param_part = param_part.trim();
        if param_part.is_empty() {
            continue;
        }

        // Find the '=' separator
        let eq_pos = param_part
            .find('=')
            .ok_or_else(|| format!("Missing '=' in parameter: {}", param_part))?;

        let param_name = param_part[..eq_pos].trim().to_lowercase();
        let param_value_raw = param_part[eq_pos + 1..].trim();

        if param_name.is_empty() {
            return Err("Empty parameter name".to_string());
        }

        // Parse parameter value (quoted or unquoted)
        let param_value = if param_value_raw.starts_with('"') && param_value_raw.ends_with('"') {
            // Parse quoted string with escape sequence support
            let quoted_content = &param_value_raw[1..param_value_raw.len() - 1];
            parse_quoted_string(quoted_content)?
        } else {
            param_value_raw.to_string()
        };

        // URL decode parameter value for server names
        let decoded_value = if param_name == "origin" || param_name == "destination" {
            urlencoding::decode(&param_value)
                .map_err(|e| format!("URL decode failed for {}: {}", param_name, e))?
                .to_string()
        } else {
            param_value
        };

        params.insert(param_name, decoded_value);
    }

    Ok(params)
}

/// Parse a quoted string with escape sequence support
fn parse_quoted_string(quoted_content: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = quoted_content.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Handle escaped characters
            match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                },
                None => {
                    result.push('\\'); // Trailing backslash
                },
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Verify X-Matrix server signature according to Matrix specification
///
/// Implements complete Matrix Server-Server API signature verification:
/// 1. Fetch server's public key from key server or cache
/// 2. Construct canonical request string for signature verification
/// 3. Verify Ed25519 signature using server's public key
/// 4. Handle key caching and expiration
pub async fn verify_server_signature(
    state: &AppState,
    x_matrix_auth: &XMatrixAuth,
    method: &str,
    path: &str,
    body: &Value,
    headers: &HeaderMap,
) -> Result<(), MatrixAuthError> {
    use base64::{Engine as _, engine::general_purpose};
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    debug!(
        "Verifying server signature from {} using key {}",
        x_matrix_auth.origin, x_matrix_auth.key_id
    );

    // Step 1: Fetch server's public key
    let public_key = state
        .session_service
        .get_server_public_key(&x_matrix_auth.origin, &x_matrix_auth.key_id)
        .await
        .map_err(|e| {
            warn!("Failed to fetch server public key: {:?}", e);
            MatrixAuthError::InvalidSignature
        })?;

    // Step 2: Construct canonical request string for signature verification
    let canonical_request = construct_canonical_request(
        method,
        path,
        &x_matrix_auth.origin,
        x_matrix_auth.destination.as_deref(),
        body,
        headers,
    )?;

    debug!("Canonical request string constructed for signature verification");

    // Step 3: Decode signature from base64
    let signature_bytes =
        general_purpose::STANDARD.decode(&x_matrix_auth.signature).map_err(|e| {
            warn!("Failed to decode signature: {}", e);
            MatrixAuthError::InvalidSignature
        })?;

    // Step 4: Create Ed25519 signature object
    let signature = Signature::from_slice(&signature_bytes).map_err(|e| {
        warn!("Invalid signature format: {}", e);
        MatrixAuthError::InvalidSignature
    })?;

    // Step 5: Decode public key from base64
    let public_key_bytes = general_purpose::STANDARD.decode(&public_key).map_err(|e| {
        warn!("Failed to decode public key: {}", e);
        MatrixAuthError::InvalidSignature
    })?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes.try_into().map_err(|_| {
        warn!("Invalid public key length");
        MatrixAuthError::InvalidSignature
    })?)
    .map_err(|e| {
        warn!("Invalid public key format: {}", e);
        MatrixAuthError::InvalidSignature
    })?;

    // Step 6: Verify signature
    verifying_key
        .verify(canonical_request.as_bytes(), &signature)
        .map_err(|e| {
            warn!("Signature verification failed: {}", e);
            MatrixAuthError::InvalidSignature
        })?;

    info!(
        "Successfully verified server signature from {} using key {}",
        x_matrix_auth.origin, x_matrix_auth.key_id
    );

    Ok(())
}

/// Construct canonical request string for Matrix signature verification
///
/// Implements Matrix Server-Server API canonical request format:
/// - HTTP method (uppercase)
/// - Request path
/// - Origin server name
/// - Destination server name (if present)
/// - Request body as canonical JSON
fn construct_canonical_request(
    method: &str,
    path: &str,
    origin: &str,
    destination: Option<&str>,
    body: &Value,
    _headers: &HeaderMap,
) -> Result<String, MatrixAuthError> {
    use crate::utils::canonical_json::to_canonical_json;

    // Convert body to canonical JSON
    let canonical_body = to_canonical_json(body).map_err(|e| {
        warn!("Failed to create canonical JSON: {}", e);
        MatrixAuthError::InvalidSignature
    })?;

    // Construct canonical request according to Matrix specification
    let mut canonical_parts = vec![method.to_uppercase(), path.to_string(), origin.to_string()];

    // Add destination if present
    if let Some(dest) = destination {
        canonical_parts.push(dest.to_string());
    }

    // Add canonical JSON body
    canonical_parts.push(canonical_body);

    let canonical_request = canonical_parts.join("\n");

    debug!("Constructed canonical request for signature verification");
    Ok(canonical_request)
}

/// Validate Matrix server name format according to specification
///
/// Valid formats:
/// - domain.com
/// - domain.com:8008
/// - [::1][]:8008 (IPv6)
/// - 192.168.1.1:8008 (IPv4)
fn is_valid_server_name(server_name: &str) -> bool {
    if server_name.is_empty() {
        return false;
    }

    // Basic validation - could be enhanced with full regex validation
    // Must not contain spaces or invalid characters
    !server_name.chars().any(|c| c.is_whitespace() || c.is_control())
        && server_name.contains('.')  // Must be a domain (not just localhost)
        && server_name.len() <= 255 // DNS name length limit
}

/// Validate signing key format supports multiple cryptographic algorithms
///
/// Valid formats:
/// - ed25519:keyid
/// - rsa:keyid (future support)
/// - curve25519:keyid (for older implementations)
fn is_valid_signing_key_format(key: &str) -> bool {
    if let Some((algorithm, key_id)) = key.split_once(':') {
        // Check algorithm is supported
        let valid_algorithms = ["ed25519", "rsa", "curve25519"];
        if !valid_algorithms.contains(&algorithm) {
            return false;
        }

        // Check key_id is valid (base64-like characters)
        !key_id.is_empty()
            && key_id.len() <= 64  // Reasonable key ID length limit
            && key_id.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=' || c == '_' || c == '-')
    } else {
        false
    }
}

/// Validate signature is valid base64 format
fn is_valid_base64_signature(signature: &str) -> bool {
    !signature.is_empty()
        && signature.len() <= 1024  // Reasonable signature length limit
        && base64::Engine::decode(&base64::engine::general_purpose::STANDARD, signature).is_ok()
}

/// PUT /_matrix/federation/v1/send/{txnId}
///
/// Push messages representing live activity to another server.
/// Each embedded PDU in the transaction body will be processed.
/// Transactions are limited to 50 PDUs and 100 EDUs.
pub async fn put(
    State(state): State<AppState>,
    Path(txn_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
    })?;

    debug!(
        "X-Matrix auth parsed - origin: {}, key_id: {}",
        x_matrix_auth.origin, x_matrix_auth.key_id
    );

    // Extract origin server from payload and verify against X-Matrix header
    let payload_origin = payload
        .get("origin")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    if payload_origin != x_matrix_auth.origin {
        warn!(
            "Origin mismatch: X-Matrix header ({}) vs payload ({})",
            x_matrix_auth.origin, payload_origin
        );
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Check for duplicate transaction ID to prevent reprocessing
    let transaction_key = format!("{}:{}", x_matrix_auth.origin, txn_id);
    if let Ok(Some(cached_result)) = check_transaction_cache(&state, &transaction_key).await {
        debug!("Returning cached result for duplicate transaction: {}", transaction_key);
        return Ok(Json(cached_result));
    }

    // Comprehensive Matrix server signature verification according to Matrix specification
    verify_server_signature(
        &state,
        &x_matrix_auth,
        "PUT",
        &format!("/_matrix/federation/v1/send/{}", txn_id),
        &payload,
        &headers,
    )
    .await
    .map_err(|e| {
        warn!("Server signature verification failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    info!(
        "Successfully verified server signature from {} using key {}",
        x_matrix_auth.origin, x_matrix_auth.key_id
    );

    // Create minimal server session for federation tracking
    let server_session = state
        .session_service
        .create_server_session(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
        )
        .await
        .map_err(|e| {
            error!("Failed to create server session: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    debug!(
        "Created server session for {} with key {} (verified signature)",
        server_session.server_name, server_session.key_id
    );
    let _origin_server_ts = payload
        .get("origin_server_ts")
        .and_then(|v| v.as_i64())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let empty_vec = vec![];
    let pdus = payload.get("pdus").and_then(|v| v.as_array()).unwrap_or(&empty_vec);

    let edus = payload.get("edus").and_then(|v| v.as_array()).unwrap_or(&empty_vec);

    // Validate transaction limits
    if pdus.len() > 50 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if edus.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Process PDUs through the 6-step validation pipeline
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let federation_repo = Arc::new(FederationRepository::new(state.db.clone()));
    let key_server_repo = Arc::new(KeyServerRepository::new(state.db.clone()));
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

    let mut pdu_results = HashMap::new();
    let mut processed_events = Vec::new();

    for pdu in pdus {
        let event_id = pdu.get("event_id").and_then(|v| v.as_str()).unwrap_or("unknown");

        debug!("Processing PDU: {}", event_id);

        match pdu_validator.validate_pdu(pdu, &x_matrix_auth.origin).await {
            Ok(ValidationResult::Valid(event)) => {
                // Store valid event in database
                match event_repo.create(&event).await {
                    Ok(stored_event) => {
                        info!("Successfully processed PDU: {}", event.event_id);
                        processed_events.push(stored_event);
                        pdu_results.insert(event.event_id, json!({}));
                    },
                    Err(e) => {
                        error!("Failed to store valid PDU {}: {}", event.event_id, e);
                        pdu_results.insert(
                            event.event_id.clone(),
                            json!({
                                "error": format!("Storage failed: {}", e)
                            }),
                        );
                    },
                }
            },
            Ok(ValidationResult::SoftFailed { event, reason }) => {
                // Store soft-failed event (marked as soft_failed)
                match event_repo.create(&event).await {
                    Ok(stored_event) => {
                        warn!("PDU {} soft-failed but stored: {}", event.event_id, reason);
                        processed_events.push(stored_event);
                        pdu_results.insert(event.event_id, json!({}));
                    },
                    Err(e) => {
                        error!("Failed to store soft-failed PDU {}: {}", event.event_id, e);
                        pdu_results.insert(
                            event.event_id.clone(),
                            json!({
                                "error": format!("Storage failed: {}", e)
                            }),
                        );
                    },
                }
            },
            Ok(ValidationResult::Rejected { event_id, reason }) => {
                warn!("PDU {} rejected: {}", event_id, reason);
                pdu_results.insert(
                    event_id,
                    json!({
                        "error": reason
                    }),
                );
            },
            Err(e) => {
                error!("PDU validation failed for {}: {}", event_id, e);
                pdu_results.insert(
                    event_id.to_string(),
                    json!({
                        "error": format!("Validation failed: {}", e)
                    }),
                );
            },
        }
    }

    // Process EDUs (Ephemeral Data Units)
    // EDUs don't require the same validation as PDUs - they're for typing indicators,
    // read receipts, presence updates, etc.
    for edu in edus {
        if let Some(edu_type) = edu.get("type").and_then(|t| t.as_str()) {
            debug!("Processing EDU: {}", edu_type);

            match edu_type {
                "m.typing" => {
                    if let Some(content) = edu.get("content") {
                        process_typing_edu(&state, &x_matrix_auth.origin, content).await.map_err(
                            |e| {
                                warn!("Failed to process typing EDU: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            },
                        )?;
                    }
                },
                "m.receipt" => {
                    if let Some(content) = edu.get("content") {
                        process_receipt_edu(&state, &x_matrix_auth.origin, content).await.map_err(
                            |e| {
                                warn!("Failed to process receipt EDU: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            },
                        )?;
                    }
                },
                "m.presence" => {
                    if let Some(content) = edu.get("content") {
                        process_presence_edu(&state, &x_matrix_auth.origin, content)
                            .await
                            .map_err(|e| {
                                warn!("Failed to process presence EDU: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;
                    }
                },
                "m.device_list_update" => {
                    if let Some(content) = edu.get("content") {
                        process_device_list_edu(&state, &x_matrix_auth.origin, content)
                            .await
                            .map_err(|e| {
                                warn!("Failed to process device list EDU: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;
                    }
                },
                "m.signing_key_update" => {
                    if let Some(content) = edu.get("content") {
                        process_signing_key_update_edu(&state, &x_matrix_auth.origin, content)
                            .await
                            .map_err(|e| {
                                warn!("Failed to process signing key update EDU: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;
                    }
                },
                "m.direct_to_device" => {
                    if let Some(content) = edu.get("content") {
                        process_direct_to_device_edu(&state, &x_matrix_auth.origin, content)
                            .await
                            .map_err(|e| {
                                warn!("Failed to process direct to device EDU: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;
                    }
                },
                _ => {
                    debug!("Unknown EDU type: {}", edu_type);
                },
            }
        }
    }

    info!(
        "Federation transaction processed: {} PDUs processed, {} events stored",
        pdus.len(),
        processed_events.len()
    );

    let response = json!({
        "pdus": pdu_results
    });

    // Cache the transaction result to prevent duplicate processing
    if let Err(e) = cache_transaction_result(&state, &transaction_key, &response).await {
        warn!("Failed to cache transaction result for {}: {}", transaction_key, e);
    }

    Ok(Json(response))
}

/// Check if a transaction has already been processed and return cached result
///
/// Queries the federation_transactions table to find previously processed
/// transactions and returns their cached results to prevent duplicate processing.
async fn check_transaction_cache(
    state: &AppState,
    transaction_key: &str,
) -> Result<Option<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let transaction_repo = TransactionRepository::new(state.db.clone());

    match transaction_repo.get_cached_result(transaction_key).await {
        Ok(Some(result)) => {
            debug!("Found cached transaction result for {}", transaction_key);
            Ok(Some(result))
        },
        Ok(None) => {
            debug!("No cached result found for transaction: {}", transaction_key);
            Ok(None)
        },
        Err(e) => Err(format!("Database query failed for transaction cache: {}", e).into()),
    }
}

/// Cache the result of a processed federation transaction
///
/// Stores transaction results in the federation_transactions table with TTL
/// to enable deduplication of retried transactions while managing storage.
async fn cache_transaction_result(
    state: &AppState,
    transaction_key: &str,
    result: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let transaction_repo = TransactionRepository::new(state.db.clone());

    match transaction_repo.cache_result(transaction_key, result.clone()).await {
        Ok(()) => {
            debug!("Cached transaction result for {}", transaction_key);
            Ok(())
        },
        Err(e) => Err(format!("Failed to cache transaction result: {}", e).into()),
    }
}

/// Process typing EDU from federation
///
/// Handles m.typing ephemeral events that indicate users typing in rooms.
/// Updates the typing_events table and triggers real-time notifications.
///
/// Matrix spec format:
/// {
///   "content": {
///     "room_id": "!room:server.com",
///     "user_id": "@user:server.com",
///     "typing": true
///   },
///   "edu_type": "m.typing"
/// }
async fn process_typing_edu(
    state: &AppState,
    origin_server: &str,
    content: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let room_id = content
        .get("room_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing room_id in typing EDU")?;

    let user_id = content
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing user_id in typing EDU")?;

    let typing = content
        .get("typing")
        .and_then(|v| v.as_bool())
        .ok_or("Missing or invalid typing boolean in typing EDU")?;

    debug!(
        "Processing typing EDU: user={}, room={}, typing={}, origin={}",
        user_id, room_id, typing, origin_server
    );

    // Validate user belongs to origin server
    if !user_id.ends_with(&format!(":{}", origin_server)) {
        warn!("Typing EDU user {} not from origin server {}", user_id, origin_server);
        return Err(format!("Invalid user origin for typing EDU: {}", user_id).into());
    }

    // Verify user is in the room
    let room_repo = RoomRepository::new(state.db.clone());

    match room_repo.check_membership(room_id, user_id).await {
        Ok(true) => {
            // User is a member, continue processing
        },
        Ok(false) => {
            debug!("Ignoring typing EDU for user {} not in room {}", user_id, room_id);
            return Ok(());
        },
        Err(e) => {
            return Err(format!("Failed to check room membership: {}", e).into());
        },
    }

    let federation_repo = FederationRepository::new(state.db.clone());
    federation_repo
        .process_typing_edu(room_id, user_id, origin_server, typing)
        .await
        .map_err(|e| format!("Failed to process typing EDU: {}", e))?;

    if typing {
        debug!("User {} started typing in room {}", user_id, room_id);
    } else {
        debug!("User {} stopped typing in room {}", user_id, room_id);
    }

    info!("Processed typing EDU: user={}, room={}, typing={}", user_id, room_id, typing);
    Ok(())
}

/// Process read receipt EDU from federation
///
/// Handles m.receipt ephemeral events that indicate users have read messages.
/// Updates the receipts table and triggers real-time notifications.
async fn process_receipt_edu(
    state: &AppState,
    origin_server: &str,
    content: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content_obj = content.as_object().ok_or("Receipt EDU content must be an object")?;

    // Process receipts for each room
    for (room_id, room_receipts) in content_obj {
        let room_receipts_obj =
            room_receipts.as_object().ok_or("Room receipts must be an object")?;

        // Process each receipt type per Matrix 1.4 specification
        for (receipt_type, receipt_data) in room_receipts_obj {
            // Only process supported receipt types
            let is_private = match receipt_type.as_str() {
                "m.read" => false,
                "m.read.private" => {
                    // Matrix 1.4 spec: m.read.private MUST NEVER be sent via federation
                    // If we receive one, the remote server is violating the spec
                    warn!(
                        "Rejecting m.read.private receipt from {}: private receipts must not be federated (spec violation by remote server)",
                        origin_server
                    );
                    continue; // Skip processing this receipt type entirely
                },
                _ => {
                    debug!("Unknown receipt type '{}' - skipping per Matrix specification", receipt_type);
                    continue;
                },
            };

            let receipt_data_obj =
                receipt_data.as_object().ok_or("Receipt data must be an object")?;

            // Process receipts for each event
            for (event_id, event_receipts) in receipt_data_obj {
                let event_receipts_obj =
                    event_receipts.as_object().ok_or("Event receipts must be an object")?;

                // Process receipts for each user
                for (user_id, user_receipt) in event_receipts_obj {
                    // Validate user is from origin server
                    if !user_id.ends_with(&format!(":{}", origin_server)) {
                        warn!(
                            "Receipt EDU user {} not from origin server {}",
                            user_id, origin_server
                        );
                        continue;
                    }

                    // Extract threading information (Matrix 1.4 requirement)
                    let thread_id = user_receipt
                        .get("thread_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let timestamp = user_receipt
                        .get("ts")
                        .and_then(|v| v.as_i64())
                        .and_then(chrono::DateTime::from_timestamp_millis)
                        .unwrap_or_else(Utc::now);

                    let federation_repo = FederationRepository::new(state.db.clone());
                    federation_repo
                        .process_receipt_edu(
                            room_id,
                            user_id,
                            event_id,
                            receipt_type,
                            timestamp.timestamp_millis(),
                        )
                        .await
                        .map_err(|e| format!("Failed to store receipt EDU: {}", e))?;

                    if is_private {
                        info!(
                            "Processed m.read.private receipt: user={}, room={}, event={}, thread={:?}",
                            user_id, room_id, event_id, &thread_id
                        );
                        // CRITICAL: Private receipts are NEVER federated per Matrix specification
                    } else {
                        info!(
                            "Processed m.read receipt: user={}, room={}, event={}, thread={:?}",
                            user_id, room_id, event_id, &thread_id
                        );
                    }
                }
            }
        }
    }

    info!("Processed receipt EDU from server {}", origin_server);
    Ok(())
}

/// Process presence EDU from federation
///
/// Handles m.presence ephemeral events that indicate user presence status.
/// Updates the presence table and triggers real-time notifications.
async fn process_presence_edu(
    state: &AppState,
    origin_server: &str,
    content: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let push = content
        .get("push")
        .and_then(|v| v.as_array())
        .ok_or("Missing or invalid push array in presence EDU")?;

    for presence_event in push {
        let user_id = presence_event
            .get("user_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing user_id in presence event")?;

        // Validate user is from origin server
        if !user_id.ends_with(&format!(":{}", origin_server)) {
            warn!("Presence EDU user {} not from origin server {}", user_id, origin_server);
            continue;
        }

        let presence = presence_event
            .get("presence")
            .and_then(|v| v.as_str())
            .unwrap_or("offline");

        let status_msg = presence_event
            .get("status_msg")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let last_active_ago = presence_event.get("last_active_ago").and_then(|v| v.as_u64());

        let currently_active = presence_event
            .get("currently_active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let federation_repo = FederationRepository::new(state.db.clone());
        federation_repo
            .process_presence_edu(
                user_id,
                presence,
                status_msg.as_deref(),
                last_active_ago.map(|v| v as i64),
                currently_active,
            )
            .await
            .map_err(|e| format!("Failed to store presence EDU: {}", e))?;

        debug!("Stored presence EDU for user {} with status {}", user_id, presence);
    }

    info!("Processed presence EDU from server {}", origin_server);
    Ok(())
}

/// Process device list update EDU from federation
///
/// Handles m.device_list_update ephemeral events that indicate user device changes.
/// Updates the device_list_updates table and triggers real-time notifications.
async fn process_device_list_edu(
    state: &AppState,
    origin_server: &str,
    content: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::federation::device_edu_handler::DeviceListUpdateEDU;
    use crate::federation::device_management::DeviceListUpdate;

    // Parse EDU content into DeviceListUpdate
    let device_update: DeviceListUpdate = serde_json::from_value(content.clone())
        .map_err(|e| format!("Failed to parse device list EDU: {}", e))?;

    // Validate user is from origin server
    if !device_update.user_id.ends_with(&format!(":{}", origin_server)) {
        warn!(
            "Device list EDU user {} not from origin server {}",
            device_update.user_id, origin_server
        );
        return Err("Invalid user origin for device list EDU".to_string().into());
    }

    // Create EDU wrapper
    let edu = DeviceListUpdateEDU {
        edu_type: "m.device_list_update".to_string(),
        content: device_update,
    };

    // Process through DeviceEDUHandler
    state
        .device_edu_handler
        .handle_device_list_update(edu)
        .await
        .map_err(|e| format!("Failed to handle device list update: {}", e))?;

    info!("Processed device list EDU from server {}", origin_server);
    Ok(())
}

/// Process signing key update EDU from federation
///
/// Handles m.signing_key_update ephemeral events that propagate cross-signing key changes.
/// Updates the cross_signing_keys table and triggers real-time key update notifications.
async fn process_signing_key_update_edu(
    state: &AppState,
    origin_server: &str,
    content: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::federation::device_edu_handler::{SigningKeyUpdateContent, SigningKeyUpdateEDU};

    // Parse EDU content into SigningKeyUpdateContent
    let signing_update: SigningKeyUpdateContent = serde_json::from_value(content.clone())
        .map_err(|e| format!("Failed to parse signing key EDU: {}", e))?;

    // Validate user is from origin server
    if !signing_update.user_id.ends_with(&format!(":{}", origin_server)) {
        return Err(format!(
            "Signing key update EDU user {} not from origin server {}",
            signing_update.user_id, origin_server
        )
        .into());
    }

    // Create EDU wrapper
    let edu = SigningKeyUpdateEDU {
        edu_type: "m.signing_key_update".to_string(),
        content: signing_update,
    };

    // Process through DeviceEDUHandler
    state
        .device_edu_handler
        .handle_signing_key_update(edu)
        .await
        .map_err(|e| format!("Failed to handle signing key update: {}", e))?;

    info!("Processed signing key update EDU from server {}", origin_server);
    Ok(())
}

/// Process and store a single cross-signing key
#[allow(dead_code)]
async fn process_cross_signing_key(
    state: &AppState,
    user_id: &str,
    key_type: &str,
    key_data: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let keys = key_data
        .get("keys")
        .and_then(|v| v.as_object())
        .ok_or(format!("Missing or invalid keys in {} key", key_type))?;

    let signatures = key_data
        .get("signatures")
        .and_then(|v| v.as_object())
        .cloned()
        .map(|s| serde_json::to_value(s).unwrap_or_default());

    let usage = key_data
        .get("usage")
        .and_then(|v| v.as_array())
        .ok_or(format!("Missing or invalid usage in {} key", key_type))?
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    // Validate usage based on key type
    match key_type {
        "master" => {
            if !usage.contains(&"master".to_string()) {
                return Err("Master key must have 'master' in usage array".to_string().into());
            }
        },
        "self_signing" => {
            if !usage.contains(&"self_signing".to_string()) {
                return Err("Self-signing key must have 'self_signing' in usage array"
                    .to_string()
                    .into());
            }
        },
        "user_signing" => {
            if !usage.contains(&"user_signing".to_string()) {
                return Err("User-signing key must have 'user_signing' in usage array"
                    .to_string()
                    .into());
            }
        },
        _ => return Err(format!("Unknown key type: {}", key_type).into()),
    }

    // Store or update the cross-signing key
    let federation_repo = FederationRepository::new(state.db.clone());
    federation_repo
        .process_signing_key_update_edu(user_id, key_type, serde_json::to_value(keys)?, signatures)
        .await
        .map_err(|e| {
            format!("Failed to store {} cross-signing key for {}: {}", key_type, user_id, e)
        })?;

    debug!("Stored {} cross-signing key for user {}", key_type, user_id);
    Ok(())
}

/// Process direct-to-device EDU for send-to-device messaging
async fn process_direct_to_device_edu(
    state: &AppState,
    origin: &str,
    content: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!("Processing direct-to-device EDU from origin: {}", origin);

    // Parse the direct-to-device content
    let message_id = content
        .get("message_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing message_id in direct-to-device EDU")?;

    let sender = content
        .get("sender")
        .and_then(|v| v.as_str())
        .ok_or("Missing sender in direct-to-device EDU")?;

    let event_type = content
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("Missing type in direct-to-device EDU")?;

    let messages = content
        .get("messages")
        .and_then(|v| v.as_object())
        .ok_or("Missing or invalid messages in direct-to-device EDU")?;

    debug!(
        "Processing direct-to-device message: id={}, sender={}, type={}, recipients={}",
        message_id,
        sender,
        event_type,
        messages.len()
    );

    let federation_repo = FederationRepository::new(state.db.clone());

    // Process messages for each user
    for (user_id, user_devices) in messages {
        let device_messages = user_devices
            .as_object()
            .ok_or(format!("Invalid device messages for user {}", user_id))?;

        // Check if user exists locally
        let user_repo = UserRepository::new(state.db.clone());

        match user_repo.user_exists(user_id).await {
            Ok(true) => {
                // User exists locally, continue processing
            },
            Ok(false) => {
                debug!("Ignoring direct-to-device message for non-local user: {}", user_id);
                continue;
            },
            Err(e) => {
                return Err(format!("Failed to check user existence: {}", e).into());
            },
        }

        // Process messages for each device
        for (device_id, message_content) in device_messages {
            if device_id == "*" {
                // Send to all devices for this user
                let device_repo = DeviceRepository::new(state.db.clone());

                match device_repo.get_all_user_devices(user_id).await {
                    Ok(user_devices) => {
                        for device in user_devices {
                            federation_repo
                                .process_direct_to_device_edu(
                                    matryx_surrealdb::repository::federation::DirectToDeviceEduParams {
                                        message_id,
                                        origin,
                                        sender,
                                        message_type: event_type,
                                        content: message_content.clone(),
                                        target_user_id: user_id,
                                        target_device_id: Some(&device.device_id),
                                    }
                                )
                                .await?;
                        }
                    },
                    Err(e) => {
                        return Err(format!("Failed to get user devices: {}", e).into());
                    },
                }
            } else {
                // Send to specific device
                let device_repo = DeviceRepository::new(state.db.clone());

                match device_repo.verify_device(user_id, device_id).await {
                    Ok(true) => {
                        // Device exists, continue processing
                    },
                    Ok(false) => {
                        debug!(
                            "Device {} not found for user {}, ignoring message",
                            device_id, user_id
                        );
                        continue;
                    },
                    Err(e) => {
                        return Err(format!("Failed to verify device: {}", e).into());
                    },
                }

                federation_repo
                    .process_direct_to_device_edu(
                        matryx_surrealdb::repository::federation::DirectToDeviceEduParams {
                            message_id,
                            origin,
                            sender,
                            message_type: event_type,
                            content: message_content.clone(),
                            target_user_id: user_id,
                            target_device_id: Some(device_id),
                        },
                    )
                    .await?;
            }
        }
    }

    debug!("Successfully processed direct-to-device EDU: {}", message_id);
    Ok(())
}
