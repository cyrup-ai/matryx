//! PDU Validation Pipeline
//!
//! Implements the complete Matrix Server-Server API PDU validation process
//! according to the 6-step validation pipeline defined in the Matrix specification.

use std::collections::HashMap;
use std::sync::Arc;

use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use crate::auth::MatrixSessionService;
use crate::federation::authorization::{AuthorizationEngine, AuthorizationError};
use crate::state::AppState;
use matryx_entity::types::Event;
use matryx_surrealdb::repository::error::RepositoryError;
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

/// Errors that can occur during PDU validation
#[derive(Debug, thiserror::Error)]
pub enum PduValidationError {
    #[error("Invalid PDU format: {0}")]
    InvalidFormat(String),

    #[error("Signature verification failed: {0}")]
    SignatureError(String),

    #[error("Hash verification failed: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("Authorization failed: {0}")]
    AuthorizationError(String),

    #[error("Matrix authorization error: {0}")]
    MatrixAuthorizationError(#[from] AuthorizationError),

    #[error("State validation failed: {0}")]
    StateError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] RepositoryError),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("Crypto error: {0}")]
    CryptoError(String),
}

/// Result of PDU validation
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Event is valid and should be accepted
    Valid(Event),

    /// Event failed validation and should be rejected
    Rejected { event_id: String, reason: String },

    /// Event should be soft-failed (stored but not used in state resolution)
    SoftFailed { event: Event, reason: String },
}

/// PDU Validator implementing Matrix Server-Server API validation pipeline
pub struct PduValidator {
    session_service: Arc<MatrixSessionService>,
    event_repo: Arc<EventRepository<surrealdb::engine::any::Any>>,
    room_repo: Arc<RoomRepository>,
    authorization_engine: AuthorizationEngine,
    db: surrealdb::Surreal<surrealdb::engine::any::Any>,
    homeserver_name: String,
}

impl PduValidator {
    pub fn new(
        session_service: Arc<MatrixSessionService>,
        event_repo: Arc<EventRepository<surrealdb::engine::any::Any>>,
        room_repo: Arc<RoomRepository>,
        db: surrealdb::Surreal<surrealdb::engine::any::Any>,
        homeserver_name: String,
    ) -> Self {
        let authorization_engine = AuthorizationEngine::new(event_repo.clone(), room_repo.clone());

        Self {
            session_service,
            event_repo,
            room_repo,
            authorization_engine,
            db,
            homeserver_name,
        }
    }

    /// Create PDU validator from application state
    pub fn from_app_state(state: &AppState) -> Self {
        let event_repo = Arc::new(EventRepository::new(state.db.clone()));
        let room_repo = Arc::new(RoomRepository::new(state.db.clone()));

        Self::new(
            state.session_service.clone(),
            event_repo,
            room_repo,
            state.db.clone(),
            state.homeserver_name.clone(),
        )
    }

    /// Validate a PDU according to the 6-step Matrix validation process
    pub async fn validate_pdu(
        &self,
        pdu: &Value,
        origin_server: &str,
    ) -> Result<ValidationResult, PduValidationError> {
        debug!("Starting PDU validation for event from server: {}", origin_server);

        // Step 1: Format Validation
        let event = self.validate_format(pdu).await?;
        debug!("Step 1 passed: Format validation for event {}", event.event_id);

        // Check for duplicate events
        if let Ok(Some(_existing)) = self.event_repo.get_by_id(&event.event_id).await {
            debug!("Event {} already exists, skipping validation", event.event_id);
            return Ok(ValidationResult::Valid(event));
        }

        // Step 2: Signature Verification
        self.validate_signatures(&event, origin_server).await?;
        debug!("Step 2 passed: Signature verification for event {}", event.event_id);

        // Step 3: Hash Verification
        self.validate_hashes(&event).await?;
        debug!("Step 3 passed: Hash verification for event {}", event.event_id);

        // Step 4: Auth Events Validation
        self.validate_auth_events(&event).await?;
        debug!("Step 4 passed: Auth events validation for event {}", event.event_id);

        // Step 5: Matrix Authorization Rules Validation (state before)
        match self.validate_matrix_authorization(&event).await {
            Ok(_) => {
                debug!(
                    "Step 5 passed: Matrix authorization validation for event {}",
                    event.event_id
                );
            },
            Err(e) => {
                warn!("Step 5 failed for event {}: {}", event.event_id, e);
                return Ok(ValidationResult::Rejected {
                    event_id: event.event_id.clone(),
                    reason: format!("Matrix authorization failed: {}", e),
                });
            },
        }

        // Step 6: Current State Validation (soft-fail check)
        match self.validate_current_state(&event).await {
            Ok(_) => {
                debug!("Step 6 passed: Current state validation for event {}", event.event_id);
                Ok(ValidationResult::Valid(event))
            },
            Err(e) => {
                warn!("Step 6 soft-failed for event {}: {}", event.event_id, e);
                let mut soft_failed_event = event;
                soft_failed_event.soft_failed = Some(true);

                Ok(ValidationResult::SoftFailed {
                    event: soft_failed_event,
                    reason: format!("Current state validation failed: {}", e),
                })
            },
        }
    }

    /// Step 1: Validate PDU format according to Matrix specification
    async fn validate_format(&self, pdu: &Value) -> Result<Event, PduValidationError> {
        let mut event: Event = serde_json::from_value(pdu.clone()).map_err(|e| {
            PduValidationError::InvalidFormat(format!("Failed to parse PDU as Event: {}", e))
        })?;

        // Basic format validation
        if event.event_id.is_empty() || !event.event_id.starts_with('$') {
            return Err(PduValidationError::InvalidFormat("Invalid event ID format".to_string()));
        }

        if event.room_id.is_empty() || !event.room_id.starts_with('!') {
            return Err(PduValidationError::InvalidFormat("Invalid room ID format".to_string()));
        }

        // Set received timestamp
        event.received_ts = Some(Utc::now().timestamp_millis());

        debug!("Format validation passed for event {}", event.event_id);
        Ok(event)
    }

    /// Step 4: Validate authorization events exist
    async fn validate_auth_events(&self, event: &Event) -> Result<(), PduValidationError> {
        if let Some(auth_events) = &event.auth_events {
            for auth_event_id in auth_events {
                if let Ok(Some(_auth_event)) = self.event_repo.get_by_id(auth_event_id).await {
                    debug!("Auth event {} exists", auth_event_id);
                } else if !event.outlier.unwrap_or(false) {
                    warn!(
                        "Auth event {} not found for non-outlier event {}",
                        auth_event_id, event.event_id
                    );
                    return Err(PduValidationError::AuthorizationError(format!(
                        "Auth event {} not found",
                        auth_event_id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Step 5: Comprehensive Matrix authorization validation using authorization engine
    async fn validate_matrix_authorization(&self, event: &Event) -> Result<(), PduValidationError> {
        // Load auth events for the authorization engine
        let mut auth_events = Vec::new();

        if let Some(auth_event_ids) = &event.auth_events {
            for auth_event_id in auth_event_ids {
                match self.event_repo.get_by_id(auth_event_id).await {
                    Ok(Some(auth_event)) => {
                        auth_events.push(auth_event);
                    },
                    Ok(None) => {
                        if !event.outlier.unwrap_or(false) {
                            return Err(PduValidationError::AuthorizationError(format!(
                                "Auth event {} not found for authorization validation",
                                auth_event_id
                            )));
                        }
                    },
                    Err(e) => {
                        error!("Database error loading auth event {}: {}", auth_event_id, e);
                        return Err(PduValidationError::DatabaseError(e));
                    },
                }
            }
        }

        // Get room version for authorization rules
        let room_version = self.get_room_version(&event.room_id).await?;

        // Run comprehensive Matrix authorization validation
        self.authorization_engine
            .authorize_event(event, &auth_events, &room_version)
            .await
            .map_err(|e| {
                debug!("Authorization failed for event {}: {}", event.event_id, e);
                PduValidationError::MatrixAuthorizationError(e)
            })?;

        debug!("Comprehensive Matrix authorization passed for event {}", event.event_id);
        Ok(())
    }

    /// Get room version for authorization validation
    async fn get_room_version(&self, room_id: &str) -> Result<String, PduValidationError> {
        // Try to get room version from the room create event
        match self.event_repo.get_room_create_event(room_id).await {
            Ok(Some(create_event)) => {
                let room_version = create_event
                    .content
                    .get("room_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1"); // Default to room version 1 if not specified

                debug!("Found room version {} for room {}", room_version, room_id);
                Ok(room_version.to_string())
            },
            Ok(None) => {
                // Room create event not found - use default room version
                warn!("Room create event not found for room {}, using default version 1", room_id);
                Ok("1".to_string())
            },
            Err(e) => {
                error!("Database error fetching room create event for {}: {}", room_id, e);
                Err(PduValidationError::DatabaseError(e))
            },
        }
    }

    /// Step 5: Basic state validation
    async fn validate_state_before(&self, event: &Event) -> Result<(), PduValidationError> {
        // Basic validation checks
        match event.event_type.as_str() {
            "m.room.member" => {
                let content = event.content.as_object().ok_or_else(|| {
                    PduValidationError::AuthorizationError(
                        "Membership event content must be object".to_string(),
                    )
                })?;

                let membership =
                    content.get("membership").and_then(|m| m.as_str()).ok_or_else(|| {
                        PduValidationError::AuthorizationError(
                            "Membership event must have membership field".to_string(),
                        )
                    })?;

                match membership {
                    "join" | "leave" | "invite" | "ban" | "knock" => Ok(()),
                    _ => {
                        Err(PduValidationError::AuthorizationError(format!(
                            "Invalid membership state: {}",
                            membership
                        )))
                    },
                }
            },
            "m.room.message" => {
                if event.content.get("body").is_none() {
                    return Err(PduValidationError::AuthorizationError(
                        "Message event must have body".to_string(),
                    ));
                }
                Ok(())
            },
            _ => {
                // Basic sender validation
                if !event.sender.starts_with('@') || !event.sender.contains(':') {
                    return Err(PduValidationError::AuthorizationError(format!(
                        "Invalid sender format: {}",
                        event.sender
                    )));
                }
                Ok(())
            },
        }
    }

    /// Step 2: Validate Ed25519 signatures using existing crypto system
    async fn validate_signatures(
        &self,
        event: &Event,
        origin_server: &str,
    ) -> Result<(), PduValidationError> {
        let signatures_value = event
            .signatures
            .as_ref()
            .map(|s| serde_json::to_value(s).unwrap_or_default())
            .unwrap_or_default();
        let signatures_obj = signatures_value.as_object().ok_or_else(|| {
            PduValidationError::SignatureError("Signatures must be an object".to_string())
        })?;

        // Verify signature from the origin server
        let server_signatures = signatures_obj
            .get(origin_server)
            .ok_or_else(|| {
                PduValidationError::SignatureError(format!(
                    "No signature found from origin server {}",
                    origin_server
                ))
            })?
            .as_object()
            .ok_or_else(|| {
                PduValidationError::SignatureError(
                    "Server signatures must be an object".to_string(),
                )
            })?;

        for (key_id, signature) in server_signatures {
            let signature_str = signature.as_str().ok_or_else(|| {
                PduValidationError::SignatureError("Signature must be a string".to_string())
            })?;

            // Create canonical JSON for signature verification
            let canonical_json = self.create_canonical_json_for_signing(event)?;

            // Get public key for the server and key ID
            let public_key = self.get_server_public_key(origin_server, key_id).await?;

            // Use existing Ed25519 verification from session service
            self.session_service
                .verify_ed25519_signature(signature_str, &canonical_json, &public_key)
                .map_err(|e| {
                    PduValidationError::SignatureError(format!(
                        "Ed25519 signature verification failed: {:?}",
                        e
                    ))
                })?;

            debug!("Verified signature from {} with key {}", origin_server, key_id);
        }

        Ok(())
    }

    /// Step 3: Validate SHA256 content hashes using existing implementation
    async fn validate_hashes(&self, event: &Event) -> Result<(), PduValidationError> {
        let hashes_value = event
            .hashes
            .as_ref()
            .map(|h| serde_json::to_value(h).unwrap_or_default())
            .unwrap_or_default();
        let hashes_obj = hashes_value.as_object().ok_or_else(|| {
            PduValidationError::InvalidFormat("Hashes must be an object".to_string())
        })?;

        // Verify SHA256 hash
        if let Some(sha256_hash) = hashes_obj.get("sha256") {
            let expected_hash = sha256_hash.as_str().ok_or_else(|| {
                PduValidationError::InvalidFormat("SHA256 hash must be a string".to_string())
            })?;

            let calculated_hashes = self.calculate_content_hashes(event)?;
            let calculated_hash =
                calculated_hashes.get("sha256").and_then(|v| v.as_str()).ok_or_else(|| {
                    PduValidationError::HashMismatch {
                        expected: expected_hash.to_string(),
                        actual: "hash calculation failed".to_string(),
                    }
                })?;

            if expected_hash != calculated_hash {
                return Err(PduValidationError::HashMismatch {
                    expected: expected_hash.to_string(),
                    actual: calculated_hash.to_string(),
                });
            }

            debug!("SHA256 hash verification passed for event {}", event.event_id);
        }

        Ok(())
    }

    /// Step 6: Validate against current room state (soft-fail check)
    async fn validate_current_state(&self, event: &Event) -> Result<(), PduValidationError> {
        // This validation can fail without rejecting the event (soft-fail)

        // Validate event depth consistency
        if event.depth.unwrap_or(0) < 0 {
            return Err(PduValidationError::StateError(
                "Event depth cannot be negative".to_string(),
            ));
        }

        // Validate prev_events exist and are reasonable
        if event.prev_events.as_ref().map_or(true, |pe| pe.is_empty()) &&
            event.depth.unwrap_or(0) > 0
        {
            return Err(PduValidationError::StateError(
                "Non-genesis events must have prev_events".to_string(),
            ));
        }

        // Check room exists locally or accept as outlier
        if let Ok(None) = self.room_repo.get_by_id(&event.room_id).await {
            debug!("Room {} not found locally, soft-failing event as outlier", event.room_id);
            return Err(PduValidationError::StateError(format!(
                "Room {} not found locally",
                event.room_id
            )));
        }

        Ok(())
    }

    /// Create canonical JSON representation for signature verification (Matrix compliant)
    fn create_canonical_json_for_signing(
        &self,
        event: &Event,
    ) -> Result<String, PduValidationError> {
        let canonical_event = json!({
            "auth_events": event.auth_events,
            "content": event.content,
            "depth": event.depth,
            "event_type": event.event_type,
            "hashes": event.hashes,
            "prev_events": event.prev_events,
            "room_id": event.room_id,
            "sender": event.sender,
            "state_key": event.state_key,
            "origin_server_ts": event.origin_server_ts
        });

        // Use Matrix canonical JSON (sorted keys, no whitespace)
        let canonical_json = self.to_canonical_json(&canonical_event)?;
        Ok(canonical_json)
    }

    /// Calculate SHA256 content hashes using existing Matrix implementation
    fn calculate_content_hashes(&self, event: &Event) -> Result<Value, PduValidationError> {
        // Create canonical JSON for content hashing per Matrix specification
        let canonical_content = json!({
            "auth_events": event.auth_events,
            "content": event.content,
            "depth": event.depth,
            "event_type": event.event_type,
            "prev_events": event.prev_events,
            "room_id": event.room_id,
            "sender": event.sender,
            "state_key": event.state_key,
            "origin_server_ts": event.origin_server_ts
        });

        // Convert to canonical JSON string (sorted keys, no whitespace)
        let canonical_json = self.to_canonical_json(&canonical_content)?;

        // Calculate SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical_json.as_bytes());
        let hash = hasher.finalize();

        // Encode as base64
        let hash_b64 = general_purpose::STANDARD.encode(&hash);

        Ok(json!({
            "sha256": hash_b64
        }))
    }

    /// Get public key for server (production implementation needed)
    async fn get_server_public_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<String, PduValidationError> {
        // For production, this needs to:
        // 1. Check local key cache in database
        // 2. If not cached, fetch from /_matrix/key/v2/server endpoint
        // 3. Verify key signatures and cache the result
        // 4. Return the base64-encoded Ed25519 public key

        // Temporary implementation - query local server keys
        let query = "
            SELECT public_key 
            FROM server_signing_keys 
            WHERE server_name = $server_name 
              AND key_id = $key_id 
              AND is_active = true
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .await
            .map_err(|e| {
                PduValidationError::CryptoError(format!("Failed to query server keys: {}", e))
            })?;

        let public_key: Option<String> = response.take(0).map_err(|e| {
            PduValidationError::CryptoError(format!(
                "Failed to parse server key query result: {}",
                e
            ))
        })?;

        match public_key {
            Some(key) => {
                debug!("Found cached public key for {}:{}", server_name, key_id);
                Ok(key)
            },
            None => {
                // Fetch server key from remote server's /_matrix/key/v2/server endpoint
                info!("Fetching server key {}:{} from remote server", server_name, key_id);
                let fetched_key = self.fetch_remote_server_key(server_name, key_id).await?;

                // Cache the fetched key for future use
                self.cache_server_key(server_name, key_id, &fetched_key).await?;

                debug!("Successfully fetched and cached server key {}:{}", server_name, key_id);
                Ok(fetched_key)
            },
        }
    }

    /// Fetch server public key from remote server's /_matrix/key/v2/server endpoint
    async fn fetch_remote_server_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<String, PduValidationError> {
        use reqwest::Client;
        use std::time::Duration;

        // Create HTTP client with appropriate timeouts
        let client = Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e| {
            PduValidationError::CryptoError(format!("Failed to create HTTP client: {}", e))
        })?;

        // Construct the server key URL
        let url = format!("https://{}/_matrix/key/v2/server", server_name);
        debug!("Fetching server keys from: {}", url);

        // Make HTTP request to fetch server keys
        let response = client
            .get(&url)
            .header("User-Agent", "matryx-homeserver/1.0")
            .send()
            .await
            .map_err(|e| {
                PduValidationError::CryptoError(format!(
                    "Failed to fetch server keys from {}: {}",
                    server_name, e
                ))
            })?;

        if !response.status().is_success() {
            return Err(PduValidationError::CryptoError(format!(
                "Server key request failed with status: {} for {}",
                response.status(),
                server_name
            )));
        }

        // Parse the JSON response
        let key_response: serde_json::Value = response.json().await.map_err(|e| {
            PduValidationError::CryptoError(format!("Failed to parse server key response: {}", e))
        })?;

        // Verify the response is for the correct server
        let response_server_name =
            key_response.get("server_name").and_then(|v| v.as_str()).ok_or_else(|| {
                PduValidationError::CryptoError(
                    "Server key response missing server_name".to_string(),
                )
            })?;

        if response_server_name != server_name {
            return Err(PduValidationError::CryptoError(format!(
                "Server key response server name mismatch: expected {}, got {}",
                server_name, response_server_name
            )));
        }

        // Check if the key response has not expired
        let valid_until_ts =
            key_response.get("valid_until_ts").and_then(|v| v.as_i64()).unwrap_or(0);

        let current_time_ms = chrono::Utc::now().timestamp_millis();
        if valid_until_ts > 0 && current_time_ms > valid_until_ts {
            return Err(PduValidationError::CryptoError(format!(
                "Server key response has expired for {}",
                server_name
            )));
        }

        // Extract the verify_keys object
        let verify_keys =
            key_response
                .get("verify_keys")
                .and_then(|v| v.as_object())
                .ok_or_else(|| {
                    PduValidationError::CryptoError(
                        "Server key response missing verify_keys".to_string(),
                    )
                })?;

        // Find the requested key_id
        let key_data = verify_keys.get(key_id).and_then(|v| v.as_object()).ok_or_else(|| {
            PduValidationError::CryptoError(format!(
                "Requested key {} not found in server response",
                key_id
            ))
        })?;

        let public_key = key_data.get("key").and_then(|v| v.as_str()).ok_or_else(|| {
            PduValidationError::CryptoError(format!("Public key data missing for key {}", key_id))
        })?;

        // Verify the signatures on the key response
        self.verify_server_key_signatures(&key_response, server_name).await?;

        debug!("Successfully fetched server key {}:{}", server_name, key_id);
        Ok(public_key.to_string())
    }

    /// Verify signatures on the server key response
    async fn verify_server_key_signatures(
        &self,
        key_response: &serde_json::Value,
        server_name: &str,
    ) -> Result<(), PduValidationError> {
        let signatures =
            key_response.get("signatures").and_then(|v| v.as_object()).ok_or_else(|| {
                PduValidationError::CryptoError(
                    "Server key response missing signatures".to_string(),
                )
            })?;

        let server_signatures =
            signatures.get(server_name).and_then(|v| v.as_object()).ok_or_else(|| {
                PduValidationError::CryptoError(format!(
                    "Server key response missing signatures from {}",
                    server_name
                ))
            })?;

        let verify_keys =
            key_response
                .get("verify_keys")
                .and_then(|v| v.as_object())
                .ok_or_else(|| {
                    PduValidationError::CryptoError(
                        "Server key response missing verify_keys for signature verification"
                            .to_string(),
                    )
                })?;

        // Create canonical JSON for signature verification (without signatures field)
        let mut key_for_signing = key_response.clone();
        if let Some(obj) = key_for_signing.as_object_mut() {
            obj.remove("signatures");
        }

        let canonical_json = self.to_canonical_json(&key_for_signing)?;

        // Verify at least one signature from the server
        let mut verified = false;
        for (signature_key_id, signature) in server_signatures {
            let signature_str = signature.as_str().ok_or_else(|| {
                PduValidationError::CryptoError("Server key signature must be a string".to_string())
            })?;

            // Get the public key for this signature
            if let Some(key_data) = verify_keys.get(signature_key_id) {
                if let Some(public_key) = key_data.get("key").and_then(|k| k.as_str()) {
                    match self.session_service.verify_ed25519_signature(
                        signature_str,
                        &canonical_json,
                        public_key,
                    ) {
                        Ok(_) => {
                            debug!(
                                "Verified server key signature from {} with key {}",
                                server_name, signature_key_id
                            );
                            verified = true;
                            break;
                        },
                        Err(e) => {
                            warn!(
                                "Failed to verify server key signature from {} with key {}: {:?}",
                                server_name, signature_key_id, e
                            );
                        },
                    }
                }
            }
        }

        if !verified {
            return Err(PduValidationError::CryptoError(format!(
                "Failed to verify any server key signatures from {}",
                server_name
            )));
        }

        Ok(())
    }

    /// Cache server public key in database for future use
    async fn cache_server_key(
        &self,
        server_name: &str,
        key_id: &str,
        public_key: &str,
    ) -> Result<(), PduValidationError> {
        let query = "
            CREATE server_signing_keys SET
                server_name = $server_name,
                key_id = $key_id,
                public_key = $public_key,
                fetched_at = $fetched_at,
                is_active = true,
                expires_at = $expires_at
        ";

        let expires_at = chrono::Utc::now() + chrono::Duration::hours(24); // Cache for 24 hours

        let _response = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .bind(("public_key", public_key.to_string()))
            .bind(("fetched_at", chrono::Utc::now()))
            .bind(("expires_at", expires_at))
            .await
            .map_err(|e| {
                PduValidationError::CryptoError(format!("Failed to cache server key: {}", e))
            })?;

        debug!("Cached server key {}:{} (expires: {})", server_name, key_id, expires_at);
        Ok(())
    }

    /// Convert JSON value to Matrix canonical JSON string with sorted keys
    ///
    /// Implements Matrix canonical JSON as defined in the Matrix specification:
    /// - Object keys sorted in lexicographic order
    /// - No insignificant whitespace
    /// - UTF-8 encoding
    /// - Numbers in shortest form
    ///
    /// This is critical for signature verification and hash calculation to work
    /// correctly with other Matrix homeservers.
    fn to_canonical_json(&self, value: &Value) -> Result<String, PduValidationError> {
        match value {
            Value::Null => Ok("null".to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Number(n) => Ok(n.to_string()),
            Value::String(s) => {
                // JSON string with proper escaping
                Ok(serde_json::to_string(s).map_err(|e| PduValidationError::JsonError(e))?)
            },
            Value::Array(arr) => {
                let elements: Result<Vec<String>, PduValidationError> =
                    arr.iter().map(|v| self.to_canonical_json(v)).collect();
                Ok(format!("[{}]", elements?.join(",")))
            },
            Value::Object(obj) => {
                // Sort keys lexicographically (critical for Matrix signature verification)
                let mut sorted_keys: Vec<&String> = obj.keys().collect();
                sorted_keys.sort();

                let pairs: Result<Vec<String>, PduValidationError> = sorted_keys
                    .into_iter()
                    .map(|key| {
                        let key_json = serde_json::to_string(key)
                            .map_err(|e| PduValidationError::JsonError(e))?;
                        let value_json = self.to_canonical_json(&obj[key])?;
                        Ok(format!("{}:{}", key_json, value_json))
                    })
                    .collect();

                Ok(format!("{{{}}}", pairs?.join(",")))
            },
        }
    }
}
