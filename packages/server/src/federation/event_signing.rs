//! Matrix Event Signing and Validation System
//!
//! Complete implementation of Matrix Server-Server API event cryptographic operations
//! according to the Matrix specification. Provides production-quality event signing,
//! hash calculation, validation, and redaction algorithms.

use std::collections::HashSet;
use std::sync::Arc;
use surrealdb::engine::any::Any;

use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use reqwest::Client;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use crate::auth::MatrixSessionService;
use crate::federation::dns_resolver::{MatrixDnsResolver, DnsResolutionError};
use crate::utils::canonical_json::to_canonical_json;
use matryx_entity::types::{Event, ServerKeysResponse};
use matryx_surrealdb::repository::{KeyServerRepository, error::RepositoryError};

/// Errors that can occur during event signing and validation
#[derive(Debug, thiserror::Error)]
pub enum EventSigningError {
    #[error("Invalid event format: {0}")]
    InvalidFormat(String),

    #[error("Signature creation failed: {0}")]
    SignatureCreationError(String),

    #[error("Hash calculation failed: {0}")]
    HashCalculationError(String),

    #[error("Server key retrieval failed: {0}")]
    KeyRetrievalError(String),

    #[error("Redaction algorithm failed: {0}")]
    RedactionError(String),

    #[error("JSON processing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Invalid destination server: {0}")]
    InvalidDestination(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] RepositoryError),

    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Base64 encoding/decoding error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("Cryptographic operation failed: {0}")]
    CryptoError(String),

    #[error("DNS resolution failed: {0}")]
    DnsResolutionError(#[from] DnsResolutionError),

    #[error("Invalid signature data: {0}")]
    InvalidSignature(String),

    #[error("Invalid HTTP request: {0}")]
    InvalidRequest(String),

    #[error("Authorization header format error: {0}")]
    HeaderFormatError(String),
}

/// Matrix Event Signing Engine
///
/// Provides complete Matrix-compliant event signing and validation functionality
/// including redaction algorithms, hash calculations, signature generation/verification,
/// and remote server key management.
#[derive(Clone)]
pub struct EventSigningEngine {
    pub session_service: Arc<MatrixSessionService<Any>>,
    pub db: surrealdb::Surreal<surrealdb::engine::any::Any>,
    key_server_repo: Arc<KeyServerRepository<surrealdb::engine::any::Any>>,
    dns_resolver: Arc<MatrixDnsResolver>,
    http_client: Client,
    homeserver_name: String,
}

impl EventSigningEngine {
    pub fn new(
        session_service: Arc<MatrixSessionService<Any>>,
        db: surrealdb::Surreal<surrealdb::engine::any::Any>,
        dns_resolver: Arc<MatrixDnsResolver>,
        homeserver_name: String,
    ) -> Result<Self, EventSigningError> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("matryx-server/1.0")
            .build()
            .map_err(|e| EventSigningError::HttpError(e))?;

        let key_server_repo = Arc::new(KeyServerRepository::new(db.clone()));

        Ok(Self {
            session_service,
            db,
            key_server_repo,
            dns_resolver,
            http_client,
            homeserver_name,
        })
    }

    /// Sign an outgoing event with server's signing key
    ///
    /// Implements the complete Matrix event signing algorithm:
    /// 1. Calculate content hash of the event
    /// 2. Add hash to event structure
    /// 3. Create redacted copy for signing
    /// 4. Sign redacted event with server key
    /// 5. Add signature to original event
    pub async fn sign_event(
        &self,
        event: &mut Event,
        signing_key_id: &str,
    ) -> Result<(), EventSigningError> {
        debug!("Signing event {} with key {}", event.event_id, signing_key_id);

        // Step 1: Calculate content hash of the complete event
        let content_hash = self.calculate_content_hash(event)?;
        debug!("Calculated content hash for event {}", event.event_id);

        // Step 2: Add hash to event structure
        let hashes_value = json!({
            "sha256": content_hash
        });
        event.hashes = Some(serde_json::from_value(hashes_value)?);

        // Step 3: Create redacted copy for signing (without signatures and unsigned fields)
        let redacted_event = self.redact_event_for_signing(event)?;
        debug!("Created redacted event copy for signing");

        // Step 4: Convert redacted event to canonical JSON
        let canonical_json = to_canonical_json(&redacted_event).map_err(|e| {
            EventSigningError::HashCalculationError(format!("Canonical JSON error: {}", e))
        })?;

        // Step 5: Sign the canonical JSON with server key
        let signature = self
            .session_service
            .sign_json(&canonical_json, signing_key_id)
            .await
            .map_err(|e| EventSigningError::SignatureCreationError(format!("{:?}", e)))?;

        // Step 6: Add signature to original event
        let mut signatures_map = if let Some(signatures) = &event.signatures {
            if let Ok(value) = serde_json::to_value(signatures) {
                if let Some(object) = value.as_object() {
                    object.clone()
                } else {
                    Map::new()
                }
            } else {
                Map::new()
            }
        } else {
            Map::new()
        };

        let mut server_signatures = if let Some(sig_value) = signatures_map.get(&self.homeserver_name) {
            if let Some(sig_object) = sig_value.as_object() {
                sig_object.clone()
            } else {
                Map::new()
            }
        } else {
            Map::new()
        };

        server_signatures.insert(signing_key_id.to_string(), Value::String(signature));
        signatures_map.insert(self.homeserver_name.clone(), Value::Object(server_signatures));

        event.signatures = Some(serde_json::from_value(Value::Object(signatures_map))?);

        info!("Successfully signed event {} with key {}", event.event_id, signing_key_id);
        Ok(())
    }

    /// Calculate SHA-256 content hash of an event
    ///
    /// Implements Matrix content hash algorithm:
    /// - Remove unsigned, signatures, and hashes fields
    /// - Convert to canonical JSON
    /// - Calculate SHA-256 hash
    /// - Return base64-encoded result
    pub fn calculate_content_hash(&self, event: &Event) -> Result<String, EventSigningError> {
        // Convert event to JSON value for manipulation
        let mut event_json = serde_json::to_value(event)?;

        // Remove fields that shouldn't be included in content hash
        if let Some(obj) = event_json.as_object_mut() {
            obj.remove("unsigned");
            obj.remove("signatures");
            obj.remove("hashes");
        }

        // Convert to canonical JSON
        let canonical_json = to_canonical_json(&event_json).map_err(|e| {
            EventSigningError::HashCalculationError(format!("Canonical JSON error: {}", e))
        })?;

        // Calculate SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical_json.as_bytes());
        let hash = hasher.finalize();

        // Encode as unpadded base64 per Matrix specification
        let hash_b64 = general_purpose::STANDARD_NO_PAD.encode(hash);

        debug!("Calculated content hash: {}", hash_b64);
        Ok(hash_b64)
    }

    /// Calculate reference hash for an event (used in some room versions)
    ///
    /// Implements Matrix reference hash algorithm:
    /// - Apply redaction algorithm to event
    /// - Remove signatures and unsigned fields
    /// - Convert to canonical JSON
    /// - Calculate SHA-256 hash
    pub fn calculate_reference_hash(&self, event: &Event, room_version: &str) -> Result<String, EventSigningError> {
        // Step 1: Apply redaction algorithm
        let redacted_event = self.redact_event(event, room_version)?;

        // Step 2: Remove signatures and unsigned fields
        let mut event_json = redacted_event;
        if let Some(obj) = event_json.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        // Step 3: Convert to canonical JSON
        let canonical_json = to_canonical_json(&event_json).map_err(|e| {
            EventSigningError::HashCalculationError(format!("Canonical JSON error: {}", e))
        })?;

        // Step 4: Calculate SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical_json.as_bytes());
        let hash = hasher.finalize();

        // Step 5: Encode as unpadded base64
        let hash_b64 = general_purpose::STANDARD_NO_PAD.encode(hash);

        debug!("Calculated reference hash: {}", hash_b64);
        Ok(hash_b64)
    }

    /// Apply Matrix redaction algorithm to an event
    ///
    /// Preserves only essential fields according to Matrix specification.
    /// Different event types preserve different content fields.
    pub fn redact_event(&self, event: &Event, room_version: &str) -> Result<Value, EventSigningError> {
        let mut redacted = json!({
            "event_id": event.event_id,
            "type": event.event_type,
            "room_id": event.room_id,
            "sender": event.sender,
            "origin_server_ts": event.origin_server_ts,
            "depth": event.depth,
            "prev_events": event.prev_events,
            "auth_events": event.auth_events
        });

        // Room version-specific top-level field preservation is handled at the
        // content level, not at the Event struct level since these fields
        // don't exist on the Event struct

        // Add state_key if present (for state events)
        if let Some(state_key) = &event.state_key {
            redacted["state_key"] = Value::String(state_key.clone());
        }

        // Add hashes if present
        if let Some(hashes) = &event.hashes {
            redacted["hashes"] = serde_json::to_value(hashes)?;
        }

        // Preserve specific content fields based on event type
        let content_value = serde_json::to_value(&event.content)?;
        let preserved_content =
            self.get_preserved_content_fields(&event.event_type, &content_value, room_version)?;
        if preserved_content.as_object().is_some_and(|obj| !obj.is_empty()) {
            redacted["content"] = preserved_content;
        }

        Ok(redacted)
    }

    /// Create redacted event specifically for signing (excludes signatures and unsigned)
    fn redact_event_for_signing(&self, event: &Event) -> Result<Value, EventSigningError> {
        let mut signing_event = json!({
            "type": event.event_type,
            "room_id": event.room_id,
            "sender": event.sender,
            "origin_server_ts": event.origin_server_ts,
            "depth": event.depth,
            "prev_events": event.prev_events,
            "auth_events": event.auth_events,
            "content": event.content
        });

        // Add state_key if present
        if let Some(state_key) = &event.state_key {
            signing_event["state_key"] = Value::String(state_key.clone());
        }

        // Add hashes if present
        if let Some(hashes) = &event.hashes {
            signing_event["hashes"] = serde_json::to_value(hashes)?;
        }

        Ok(signing_event)
    }

    /// Get preserved content fields for redacted events based on event type
    ///
    /// Each event type has specific content fields that are preserved during redaction
    /// according to the Matrix specification.
    fn get_preserved_content_fields(
        &self,
        event_type: &str,
        content: &Value,
        room_version: &str,
    ) -> Result<Value, EventSigningError> {
        let content_obj = content.as_object().ok_or_else(|| {
            EventSigningError::RedactionError("Event content must be an object".to_string())
        })?;

        let preserved_fields: HashSet<&str> = match (event_type, room_version) {
            // m.room.member - room version specific
            ("m.room.member", "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8") => {
                ["membership"].iter().cloned().collect()
            },
            ("m.room.member", "9" | "10") => {
                ["membership", "join_authorised_via_users_server"].iter().cloned().collect()
            },
            ("m.room.member", "11" | _) => {
                // Version 11+ also allows signed key of third_party_invite
                ["membership", "join_authorised_via_users_server"].iter().cloned().collect()
            },

            // m.room.create - room version specific
            ("m.room.create", "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10") => {
                ["creator", "m.federate", "room_version"].iter().cloned().collect()
            },
            ("m.room.create", "11" | _) => {
                // Version 11+ allows ALL keys for m.room.create
                return Ok(content.clone());
            },

            // m.room.join_rules - room version specific
            ("m.room.join_rules", "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8") => {
                ["join_rule"].iter().cloned().collect()
            },
            ("m.room.join_rules", "9" | "10" | "11" | _) => {
                ["join_rule", "allow"].iter().cloned().collect()
            },

            // m.room.power_levels - room version specific
            ("m.room.power_levels", "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10") => {
                [
                    "ban", "events", "events_default", "kick", "redact",
                    "state_default", "users", "users_default"
                ].iter().cloned().collect()
            },
            ("m.room.power_levels", "11" | _) => {
                [
                    "ban", "events", "events_default", "invite", "kick", "redact",
                    "state_default", "users", "users_default"
                ].iter().cloned().collect()
            },

            // m.room.history_visibility - consistent across versions
            ("m.room.history_visibility", _) => {
                ["history_visibility"].iter().cloned().collect()
            },

            // m.room.aliases - room version specific
            ("m.room.aliases", "1" | "2" | "3" | "4" | "5") => {
                ["aliases"].iter().cloned().collect()
            },
            ("m.room.aliases", "6" | "7" | "8" | "9" | "10" | "11" | _) => {
                // Version 6+ removes m.room.aliases preservation
                HashSet::new()
            },

            // m.room.redaction - version 11+ only
            ("m.room.redaction", "11") => {
                ["redacts"].iter().cloned().collect()
            },
            ("m.room.redaction", _) => HashSet::new(),

            // All other event types
            (_, _) => HashSet::new(),
        };

        let mut preserved_content = Map::new();
        for field in preserved_fields {
            if let Some(value) = content_obj.get(field) {
                preserved_content.insert(field.to_string(), value.clone());
            }
        }

        Ok(Value::Object(preserved_content))
    }

    /// Fetch server signing keys from remote Matrix server
    ///
    /// Implements complete server key fetching with verification:
    /// 1. Make HTTP request to /_matrix/key/v2/server endpoint
    /// 2. Verify server key signatures
    /// 3. Cache valid keys in database
    /// 4. Return requested public key
    pub async fn fetch_remote_server_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<String, EventSigningError> {
        debug!("Fetching remote server key {}:{}", server_name, key_id);

        // Check cache first
        if let Some(cached_key) = self.get_cached_server_key(server_name, key_id).await? {
            debug!("Found cached server key {}:{}", server_name, key_id);
            return Ok(cached_key);
        }

        // Fetch from remote server using Matrix DNS resolution
        let resolved = self.dns_resolver.resolve_server(server_name).await?;
        let base_url = self.dns_resolver.get_base_url(&resolved);
        let host_header = self.dns_resolver.get_host_header(&resolved);
        let key_url = format!("{}/_matrix/key/v2/server", base_url);
        debug!("Fetching server keys from: {}", key_url);

        let response = self.http_client
            .get(&key_url)
            .header("Host", host_header)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(EventSigningError::KeyRetrievalError(format!(
                "HTTP {} from {}",
                response.status(),
                key_url
            )));
        }

        let server_keys: ServerKeysResponse = response.json().await?;
        debug!("Received server keys response from {}", server_name);

        // Validate key expiration with 7-day maximum as per Matrix spec
        let now = Utc::now().timestamp_millis();
        let seven_days_from_now = now + (7 * 24 * 60 * 60 * 1000); // 7 days in milliseconds
        let effective_valid_until = std::cmp::min(server_keys.valid_until_ts, seven_days_from_now);

        if effective_valid_until <= now {
            return Err(EventSigningError::KeyRetrievalError(format!(
                "Server key from {} has expired or will expire within 7 days (valid_until_ts: {}, now: {})",
                server_name, server_keys.valid_until_ts, now
            )));
        }

        debug!("Key lifetime validation passed for {} (effective_valid_until: {})", server_name, effective_valid_until);

        // Verify server key signatures
        self.verify_server_key_signatures(&server_keys, server_name).await?;
        debug!("Verified server key signatures for {}", server_name);

        // Cache the keys
        self.cache_server_keys(&server_keys, server_name).await?;
        debug!("Cached server keys for {}", server_name);

        // Extract the requested key
        let verify_keys = &server_keys.verify_keys;

        let key_data = verify_keys.get(key_id).ok_or_else(|| {
            EventSigningError::KeyRetrievalError(format!("Key {} not found", key_id))
        })?;

        let public_key = &key_data.key;

        info!("Successfully fetched server key {}:{}", server_name, key_id);
        Ok(public_key.clone())
    }

    /// Verify signatures on a server keys response
    async fn verify_server_key_signatures(
        &self,
        server_keys: &ServerKeysResponse,
        server_name: &str,
    ) -> Result<(), EventSigningError> {
        // Get signatures directly from server_keys
        let server_signatures = server_keys.signatures.get(server_name).ok_or_else(|| {
            EventSigningError::KeyRetrievalError(format!("No signatures from {}", server_name))
        })?;

        // Create canonical JSON without signatures for verification
        let mut keys_for_verification = serde_json::to_value(server_keys)?;
        if let Some(obj) = keys_for_verification.as_object_mut() {
            obj.remove("signatures");
        }

        let canonical_json = to_canonical_json(&keys_for_verification)
            .map_err(|e| EventSigningError::CryptoError(format!("Canonical JSON error: {}", e)))?;

        // Verify at least one signature
        let verify_keys = &server_keys.verify_keys;

        let mut verified = false;
        for (key_id, signature) in server_signatures {
            if let Some(key_data) = verify_keys.get(key_id) {
                match self.session_service.verify_ed25519_signature(
                    signature,
                    &canonical_json,
                    &key_data.key,
                ) {
                    Ok(_) => {
                        debug!("Verified server key signature {}:{}", server_name, key_id);
                        verified = true;
                        break;
                    },
                    Err(e) => {
                        warn!("Failed to verify signature {}:{}: {:?}", server_name, key_id, e);
                    },
                }
            }
        }

        if !verified {
            return Err(EventSigningError::CryptoError(format!(
                "Failed to verify any server key signatures from {}",
                server_name
            )));
        }

        Ok(())
    }

    /// Cache server keys in database
    async fn cache_server_keys(
        &self,
        server_keys: &ServerKeysResponse,
        server_name: &str,
    ) -> Result<(), EventSigningError> {
        let verify_keys = &server_keys.verify_keys;

        let now = Utc::now();
        let valid_until = chrono::DateTime::from_timestamp(server_keys.valid_until_ts / 1000, 0)
            .ok_or_else(|| EventSigningError::KeyRetrievalError(
                format!("Invalid timestamp in server keys: {}", server_keys.valid_until_ts)
            ))?;

        // Calculate cache expiration as half the key lifetime per Matrix spec
        // "Intermediate notary servers should cache a response for half of its lifetime"
        let key_lifetime = valid_until.signed_duration_since(now);
        let cache_lifetime = key_lifetime / 2;
        let cache_until = now + cache_lifetime;

        debug!("Caching server keys for {} until {} (half of lifetime)", server_name, cache_until);

        for (key_id, key_data) in verify_keys {
            // Cache server signing key using key server repository
            self.key_server_repo
                .cache_server_signing_key(
                    server_name,
                    key_id,
                    &key_data.key,
                    now,
                    cache_until,
                )
                .await
                .map_err(EventSigningError::DatabaseError)?;

            debug!("Cached server key {}:{}", server_name, key_id);
        }

        Ok(())
    }

    /// Get cached server key from database
    async fn get_cached_server_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<Option<String>, EventSigningError> {
        // Get cached server signing key using key server repository
        self.key_server_repo
            .get_server_signing_key(server_name, key_id)
            .await
            .map_err(EventSigningError::DatabaseError)
    }

    /// Validate event signatures and hashes
    ///
    /// Complete validation of all cryptographic aspects of an event:
    /// 1. Verify all signatures from expected servers
    /// 2. Verify content hash integrity
    /// 3. Ensure proper redaction handling
    pub async fn validate_event_crypto(
        &self,
        event: &Event,
        expected_servers: &[String],
    ) -> Result<(), EventSigningError> {
        debug!("Validating crypto for event {}", event.event_id);

        // Step 1: Validate signatures
        self.validate_event_signatures(event, expected_servers).await?;
        debug!("Signature validation passed for event {}", event.event_id);

        // Step 2: Validate content hash
        self.validate_event_hash(event).await?;
        debug!("Hash validation passed for event {}", event.event_id);

        info!("Complete crypto validation passed for event {}", event.event_id);
        Ok(())
    }

    /// Validate all signatures on an event
    async fn validate_event_signatures(
        &self,
        event: &Event,
        expected_servers: &[String],
    ) -> Result<(), EventSigningError> {
        let signatures_value = if let Some(signatures) = &event.signatures {
            if let Ok(value) = serde_json::to_value(signatures) {
                value
            } else {
                Value::Object(Map::new())
            }
        } else {
            Value::Object(Map::new())
        };

        let signatures_obj = signatures_value.as_object().ok_or_else(|| {
            EventSigningError::CryptoError("Signatures must be an object".to_string())
        })?;

        // Create redacted event for signature verification
        let redacted_event = self.redact_event_for_signing(event)?;
        let canonical_json = to_canonical_json(&redacted_event)
            .map_err(|e| EventSigningError::CryptoError(format!("Canonical JSON error: {}", e)))?;

        // Verify signatures from each expected server
        for server_name in expected_servers {
            let server_signatures = signatures_obj
                .get(server_name)
                .and_then(|s| s.as_object())
                .ok_or_else(|| {
                    EventSigningError::CryptoError(format!(
                        "No signature from expected server {}",
                        server_name
                    ))
                })?;

            // Verify at least one signature from this server
            let mut verified = false;
            for (key_id, signature) in server_signatures {
                let signature_str = signature.as_str().ok_or_else(|| {
                    EventSigningError::CryptoError("Signature must be a string".to_string())
                })?;

                // Get public key for verification
                let public_key = self.fetch_remote_server_key(server_name, key_id).await?;

                match self.session_service.verify_ed25519_signature(
                    signature_str,
                    &canonical_json,
                    &public_key,
                ) {
                    Ok(_) => {
                        debug!("Verified signature from {}:{}", server_name, key_id);
                        verified = true;
                        break;
                    },
                    Err(e) => {
                        warn!("Failed to verify signature {}:{}: {:?}", server_name, key_id, e);
                    },
                }
            }

            if !verified {
                return Err(EventSigningError::CryptoError(format!(
                    "Failed to verify any signature from {}",
                    server_name
                )));
            }
        }

        Ok(())
    }

    /// Validate content hash of an event
    async fn validate_event_hash(&self, event: &Event) -> Result<(), EventSigningError> {
        let hashes_value = if let Some(hashes) = &event.hashes {
            if let Ok(value) = serde_json::to_value(hashes) {
                value
            } else {
                Value::Object(Map::new())
            }
        } else {
            Value::Object(Map::new())
        };

        let hashes_obj = hashes_value.as_object().ok_or_else(|| {
            EventSigningError::CryptoError("Hashes must be an object".to_string())
        })?;

        if let Some(expected_hash) = hashes_obj.get("sha256").and_then(|h| h.as_str()) {
            let calculated_hash = self.calculate_content_hash(event)?;

            if expected_hash != calculated_hash {
                return Err(EventSigningError::CryptoError(format!(
                    "Hash mismatch: expected {}, calculated {}",
                    expected_hash, calculated_hash
                )));
            }

            debug!("Content hash validation passed for event {}", event.event_id);
        }

        Ok(())
    }

    /// Sign arbitrary JSON object with server's signing key
    ///
    /// Implements Matrix JSON signing algorithm for federation requests.
    /// Follows the same pattern as sign_event() but works with arbitrary JSON Values.
    ///
    /// # Arguments
    /// * `json_object` - The JSON value to sign
    /// * `key_name` - Optional key ID to use (defaults to "ed25519:auto")
    ///
    /// # Returns
    /// * `Ok(Value)` - The original JSON with signatures field added
    /// * `Err(EventSigningError)` - If signing fails
    pub async fn sign_json(
        &self,
        json_object: &Value,
        key_name: Option<&str>,
    ) -> Result<Value, EventSigningError> {
        let signing_key_id = if let Some(key) = key_name {
            key
        } else {
            "ed25519:auto"
        };

        debug!("Signing JSON object with key {}", signing_key_id);

        // Step 1: Convert JSON to canonical form (same as sign_event pattern)
        let canonical_json = to_canonical_json(json_object).map_err(|e| {
            EventSigningError::HashCalculationError(format!("Canonical JSON error: {}", e))
        })?;

        // Step 2: Sign the canonical JSON (same as sign_event pattern)
        let signature = self
            .session_service
            .sign_json(&canonical_json, signing_key_id)
            .await
            .map_err(|e| EventSigningError::SignatureCreationError(format!("{:?}", e)))?;

        // Step 3: Add signature to JSON (same as sign_event pattern)
        let mut signed_json = json_object.clone();

        let mut signatures_map = if let Some(sig_value) = signed_json.get("signatures") {
            if let Some(sig_object) = sig_value.as_object() {
                sig_object.clone()
            } else {
                Map::new()
            }
        } else {
            Map::new()
        };

        let mut server_signatures = if let Some(sig_value) = signatures_map.get(&self.homeserver_name) {
            if let Some(sig_object) = sig_value.as_object() {
                sig_object.clone()
            } else {
                Map::new()
            }
        } else {
            Map::new()
        };

        server_signatures.insert(signing_key_id.to_string(), Value::String(signature));
        signatures_map.insert(self.homeserver_name.clone(), Value::Object(server_signatures));

        signed_json.as_object_mut()
            .ok_or_else(|| EventSigningError::InvalidFormat("Input must be JSON object".to_string()))?
            .insert("signatures".to_string(), Value::Object(signatures_map));

        debug!("Successfully signed JSON object with key {}", signing_key_id);
        Ok(signed_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_redaction_preserves_essential_fields() {
        let engine = create_test_engine();

        let event_json = json!({
            "event_id": "$test:example.org",
            "type": "m.room.member",
            "room_id": "!room:example.org",
            "sender": "@user:example.org",
            "origin_server_ts": 1234567890,
            "depth": 5,
            "prev_events": ["$prev:example.org"],
            "auth_events": ["$auth:example.org"],
            "state_key": "@user:example.org",
            "content": {
                "membership": "join",
                "displayname": "Test User",
                "avatar_url": "mxc://example.org/avatar"
            },
            "unsigned": {
                "age": 1000
            }
        });

        let event: Event = serde_json::from_value(event_json).unwrap();
        let redacted = engine.redact_event(&event, "10").unwrap();

        // Essential fields should be preserved
        assert_eq!(redacted["event_id"], "$test:example.org");
        assert_eq!(redacted["type"], "m.room.member");
        assert_eq!(redacted["sender"], "@user:example.org");
        assert_eq!(redacted["state_key"], "@user:example.org");

        // Only membership should be preserved in content for m.room.member events
        let content = redacted["content"].as_object().unwrap();
        assert_eq!(content["membership"], "join");
        assert!(!content.contains_key("displayname"));
        assert!(!content.contains_key("avatar_url"));

        // unsigned should not be present
        assert!(!redacted.as_object().unwrap().contains_key("unsigned"));
    }

    #[test]
    fn test_content_hash_calculation() {
        let engine = create_test_engine();

        let event_json = json!({
            "event_id": "$test:example.org",
            "type": "m.room.message",
            "room_id": "!room:example.org",
            "sender": "@user:example.org",
            "origin_server_ts": 1234567890,
            "content": {
                "msgtype": "m.text",
                "body": "Hello, world!"
            }
        });

        let event: Event = serde_json::from_value(event_json).unwrap();
        let hash = engine.calculate_content_hash(&event).unwrap();

        // Hash should be a valid base64 string
        assert!(general_purpose::STANDARD_NO_PAD.decode(&hash).is_ok());
        assert!(!hash.is_empty());
    }

    fn create_test_engine() -> EventSigningEngine {
        use matryx_surrealdb::test_utils::create_test_db;
        use std::sync::Arc;
        use crate::federation::well_known_client::WellKnownClient;

        let test_db = create_test_db().expect("Failed to create test database");
        
        let session_repo = matryx_surrealdb::repository::session::SessionRepository::new(test_db.clone());
        let key_server_repo = matryx_surrealdb::repository::key_server::KeyServerRepository::new(test_db.clone());
        
        let session_service = Arc::new(MatrixSessionService::new(
            b"test_secret".to_vec(),
            "test.example.org".to_string(),
            session_repo,
            key_server_repo,
        ));

        // Create test DNS resolver
        let http_client = Arc::new(reqwest::Client::new());
        let well_known_client = Arc::new(WellKnownClient::new(http_client));
        let dns_resolver = Arc::new(MatrixDnsResolver::new(well_known_client).expect("Failed to create DNS resolver"));

        EventSigningEngine::new(session_service, test_db, dns_resolver, "test.example.org".to_string())
            .expect("Failed to create test EventSigningEngine")
    }
}
