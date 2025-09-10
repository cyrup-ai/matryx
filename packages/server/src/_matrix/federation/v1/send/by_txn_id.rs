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

use crate::federation::pdu_validator::{PduValidator, ValidationResult};
use crate::state::AppState;
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

/// Matrix X-Matrix authentication header parsed structure
#[derive(Debug, Clone)]
struct XMatrixAuth {
    origin: String,
    key_id: String,
    signature: String,
}

/// Parse X-Matrix authentication header
///
/// Extracts origin, key_id, and signature from the Authorization header
/// Format: X-Matrix origin=origin.server,key="ed25519:key_id",sig="signature"
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
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
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

    // Create server token for federation using parsed values
    let _server_token = state
        .session_service
        .create_server_token(&x_matrix_auth.origin, &x_matrix_auth.key_id, 300)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Create server session for federation using parsed values
    let server_session = state
        .session_service
        .create_server_session(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
        )
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Access the fields to ensure they're used
    let _server_name_field = &server_session.server_name;
    let _key_id_field = &server_session.key_id;
    let _signature_field = &server_session.signature;

    // Use session service to validate server signature with parsed values and actual payload
    let request_body = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize request payload for signature verification: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "PUT",
            "/send",
            &request_body,
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;
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

    let pdu_validator = PduValidator::new(
        state.session_service.clone(),
        event_repo.clone(),
        room_repo.clone(),
        state.db.clone(),
        state.homeserver_name.clone(),
    );

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
    let query = "
        SELECT result, created_at
        FROM federation_transactions
        WHERE transaction_key = $transaction_key
        ORDER BY created_at DESC
        LIMIT 1
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("transaction_key", transaction_key.to_string()))
        .await
        .map_err(|e| format!("Database query failed for transaction cache: {}", e))?;

    #[derive(serde::Deserialize)]
    struct TransactionRecord {
        result: Value,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let transaction_record: Option<TransactionRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse transaction cache query result: {}", e))?;

    match transaction_record {
        Some(record) => {
            debug!(
                "Found cached transaction result for {} from {}",
                transaction_key, record.created_at
            );
            Ok(Some(record.result))
        },
        None => {
            debug!("No cached result found for transaction: {}", transaction_key);
            Ok(None)
        },
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
    let query = "
        CREATE federation_transactions SET
            transaction_key = $transaction_key,
            result = $result,
            created_at = $created_at,
            expires_at = $expires_at
    ";

    let now = Utc::now();
    let expires_at = now + chrono::Duration::hours(24); // Cache for 24 hours

    let _response = state
        .db
        .query(query)
        .bind(("transaction_key", transaction_key.to_string()))
        .bind(("result", result.clone()))
        .bind(("created_at", now))
        .bind(("expires_at", expires_at))
        .await
        .map_err(|e| format!("Failed to cache transaction result: {}", e))?;

    debug!("Cached transaction result for {} (expires: {})", transaction_key, expires_at);

    Ok(())
}

/// Process typing EDU from federation
///
/// Handles m.typing ephemeral events that indicate users typing in rooms.
/// Updates the typing_events table and triggers real-time notifications.
async fn process_typing_edu(
    state: &AppState,
    origin_server: &str,
    content: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let room_id = content
        .get("room_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing room_id in typing EDU")?;

    let user_ids = content
        .get("user_ids")
        .and_then(|v| v.as_array())
        .ok_or("Missing or invalid user_ids in typing EDU")?;

    let timeout = content.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30000); // Default 30 seconds

    // Validate users are from the origin server
    for user_id_val in user_ids {
        if let Some(user_id) = user_id_val.as_str() {
            if !user_id.ends_with(&format!(":{}", origin_server)) {
                warn!("Typing EDU user {} not from origin server {}", user_id, origin_server);
                return Err(format!("Invalid user origin for typing EDU: {}", user_id).into());
            }
        }
    }

    let expires_at = Utc::now() + chrono::Duration::milliseconds(timeout as i64);

    // Update typing events table
    for user_id_val in user_ids {
        if let Some(user_id) = user_id_val.as_str() {
            let query = "
                CREATE typing_events SET
                    room_id = $room_id,
                    user_id = $user_id,
                    server_name = $server_name,
                    started_at = $started_at,
                    expires_at = $expires_at
            ";

            let _response = state
                .db
                .query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("user_id", user_id.to_string()))
                .bind(("server_name", origin_server.to_string()))
                .bind(("started_at", Utc::now()))
                .bind(("expires_at", expires_at))
                .await
                .map_err(|e| format!("Failed to store typing EDU: {}", e))?;

            debug!(
                "Stored typing EDU for user {} in room {} (expires: {})",
                user_id, room_id, expires_at
            );
        }
    }

    info!("Processed typing EDU for room {} from server {}", room_id, origin_server);
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

        // Process each receipt type (currently only m.read is standard)
        for (receipt_type, receipt_data) in room_receipts_obj {
            if receipt_type != "m.read" {
                continue; // Only handle read receipts for now
            }

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

                    let timestamp = user_receipt
                        .get("ts")
                        .and_then(|v| v.as_i64())
                        .map(|ts| chrono::DateTime::from_timestamp_millis(ts))
                        .flatten()
                        .unwrap_or_else(Utc::now);

                    let query = "
                        CREATE receipts SET
                            room_id = $room_id,
                            user_id = $user_id,
                            event_id = $event_id,
                            receipt_type = $receipt_type,
                            timestamp = $timestamp,
                            server_name = $server_name,
                            received_at = $received_at
                    ";

                    let _response = state
                        .db
                        .query(query)
                        .bind(("room_id", room_id.to_string()))
                        .bind(("user_id", user_id.to_string()))
                        .bind(("event_id", event_id.to_string()))
                        .bind(("receipt_type", receipt_type.to_string()))
                        .bind(("timestamp", timestamp))
                        .bind(("server_name", origin_server.to_string()))
                        .bind(("received_at", Utc::now()))
                        .await
                        .map_err(|e| format!("Failed to store receipt EDU: {}", e))?;

                    debug!(
                        "Stored receipt EDU for user {} on event {} in room {}",
                        user_id, event_id, room_id
                    );
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

        let query = "
            CREATE presence_events SET
                user_id = $user_id,
                presence = $presence,
                status_msg = $status_msg,
                last_active_ago = $last_active_ago,
                currently_active = $currently_active,
                server_name = $server_name,
                updated_at = $updated_at
        ";

        let _response = state
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("presence", presence.to_string()))
            .bind(("status_msg", status_msg))
            .bind(("last_active_ago", last_active_ago.map(|v| v as i64)))
            .bind(("currently_active", currently_active))
            .bind(("server_name", origin_server.to_string()))
            .bind(("updated_at", Utc::now()))
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
    let user_id = content
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing user_id in device list EDU")?;

    // Validate user is from origin server
    if !user_id.ends_with(&format!(":{}", origin_server)) {
        warn!("Device list EDU user {} not from origin server {}", user_id, origin_server);
        return Err(format!("Invalid user origin for device list EDU: {}", user_id).into());
    }

    let device_id = content
        .get("device_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing device_id in device list EDU")?;

    let stream_id = content.get("stream_id").and_then(|v| v.as_i64()).unwrap_or(0);

    let prev_id = content
        .get("prev_id")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>())
        .unwrap_or_default();

    let deleted = content.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false);

    let device_display_name = content
        .get("device_display_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let keys = content.get("keys").map(|v| v.clone());

    let query = "
        CREATE device_list_updates SET
            user_id = $user_id,
            device_id = $device_id,
            stream_id = $stream_id,
            prev_id = $prev_id,
            deleted = $deleted,
            device_display_name = $device_display_name,
            keys = $keys,
            server_name = $server_name,
            received_at = $received_at
    ";

    let _response = state
        .db
        .query(query)
        .bind(("user_id", user_id.to_string()))
        .bind(("device_id", device_id.to_string()))
        .bind(("stream_id", stream_id))
        .bind(("prev_id", prev_id))
        .bind(("deleted", deleted))
        .bind(("device_display_name", device_display_name))
        .bind(("keys", keys))
        .bind(("server_name", origin_server.to_string()))
        .bind(("received_at", Utc::now()))
        .await
        .map_err(|e| format!("Failed to store device list EDU: {}", e))?;

    debug!(
        "Stored device list EDU for user {} device {} (deleted: {})",
        user_id, device_id, deleted
    );

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
    let user_id = content
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing user_id in signing key update EDU")?;

    // Validate user is from origin server
    if !user_id.ends_with(&format!(":{}", origin_server)) {
        return Err(format!(
            "Signing key update EDU user {} not from origin server {}",
            user_id, origin_server
        )
        .into());
    }

    let master_key = content.get("master_key");
    let self_signing_key = content.get("self_signing_key");
    let user_signing_key = content.get("user_signing_key");

    // Process each key type that's present
    if let Some(master_key_data) = master_key {
        process_cross_signing_key(state, user_id, "master", master_key_data).await?;
    }

    if let Some(self_signing_key_data) = self_signing_key {
        process_cross_signing_key(state, user_id, "self_signing", self_signing_key_data).await?;
    }

    if let Some(user_signing_key_data) = user_signing_key {
        process_cross_signing_key(state, user_id, "user_signing", user_signing_key_data).await?;
    }

    info!("Processed signing key update EDU for user {} from server {}", user_id, origin_server);
    Ok(())
}

/// Process and store a single cross-signing key
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

    let signatures = key_data.get("signatures").and_then(|v| v.as_object()).cloned();

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
                return Err(format!("Master key must have 'master' in usage array").into());
            }
        },
        "self_signing" => {
            if !usage.contains(&"self_signing".to_string()) {
                return Err(
                    format!("Self-signing key must have 'self_signing' in usage array").into()
                );
            }
        },
        "user_signing" => {
            if !usage.contains(&"user_signing".to_string()) {
                return Err(
                    format!("User-signing key must have 'user_signing' in usage array").into()
                );
            }
        },
        _ => return Err(format!("Unknown key type: {}", key_type).into()),
    }

    // Store or update the cross-signing key
    let query = "
        BEGIN;
        DELETE cross_signing_keys WHERE user_id = $user_id AND key_type = $key_type;
        CREATE cross_signing_keys SET
            user_id = $user_id,
            key_type = $key_type,
            keys = $keys,
            signatures = $signatures,
            usage = $usage,
            updated_at = $updated_at;
        COMMIT;
    ";

    let _response = state
        .db
        .query(query)
        .bind(("user_id", user_id.to_string()))
        .bind(("key_type", key_type.to_string()))
        .bind(("keys", serde_json::to_value(keys)?))
        .bind(("signatures", serde_json::to_value(signatures)?))
        .bind(("usage", usage))
        .bind(("updated_at", Utc::now()))
        .await
        .map_err(|e| {
            format!("Failed to store {} cross-signing key for {}: {}", key_type, user_id, e)
        })?;

    debug!("Stored {} cross-signing key for user {}", key_type, user_id);
    Ok(())
}
