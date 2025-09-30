//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

//! Event Signing Utility for Matrix Server
//!
//! Provides high-level interface for signing outgoing Matrix events
//! before sending them over federation. Integrates with the complete
//! EventSigningEngine to ensure proper Matrix specification compliance.

use std::sync::Arc;
use surrealdb::engine::any::Any;

use tracing::{debug, info, warn};

use crate::auth::MatrixSessionService;
use crate::auth::signing::FederationRequestSigner;
use crate::federation::dns_resolver::MatrixDnsResolver;
use crate::federation::event_signing::{EventSigningEngine, EventSigningError};
use matryx_entity::types::Event;
use matryx_surrealdb::repository::{KeyServerRepository, key_server::SigningKey};

/// High-level event signing service for outgoing Matrix events
///
/// Provides a simplified interface for signing events before federation,
/// automatically handling key selection, hash calculation, and signature
/// generation according to the Matrix specification.
pub struct EventSigner {
    signing_engine: EventSigningEngine,
    key_server_repo: Arc<KeyServerRepository<surrealdb::engine::any::Any>>,
    default_key_id: String,
    homeserver_name: String,
}

impl EventSigner {
    pub fn new(
        session_service: Arc<MatrixSessionService<Any>>,
        db: surrealdb::Surreal<surrealdb::engine::any::Any>,
        dns_resolver: Arc<MatrixDnsResolver>,
        homeserver_name: String,
        default_key_id: String,
    ) -> Result<Self, EventSigningError> {
        let signing_engine =
            EventSigningEngine::new(session_service, db.clone(), dns_resolver, homeserver_name.clone())?;
        let key_server_repo = Arc::new(KeyServerRepository::new(db));

        Ok(Self {
            signing_engine,
            key_server_repo,
            default_key_id,
            homeserver_name,
        })
    }

    /// Sign an outgoing event for federation
    ///
    /// This is the primary method for signing events before sending them
    /// to other Matrix servers. It handles all the cryptographic operations
    /// required by the Matrix specification.
    ///
    /// # Arguments
    /// * `event` - Mutable reference to the event to sign
    /// * `key_id` - Optional specific key ID to use (defaults to server's default)
    ///
    /// # Returns
    /// * `Ok(())` if signing succeeds
    /// * `Err(EventSigningError)` if any step fails
    pub async fn sign_outgoing_event(
        &self,
        event: &mut Event,
        key_id: Option<&str>,
    ) -> Result<(), EventSigningError> {
        let signing_key = key_id.unwrap_or(&self.default_key_id);

        debug!("Signing outgoing event {} with key {}", event.event_id, signing_key);

        // Validate event has required fields before signing
        self.validate_event_for_signing(event)?;

        // Sign the event using the complete signing engine
        self.signing_engine.sign_event(event, signing_key).await?;

        info!(
            "Successfully signed outgoing event {} from {} for room {}",
            event.event_id, event.sender, event.room_id
        );

        Ok(())
    }

    /// Sign multiple events in batch for efficient federation
    ///
    /// Useful for signing multiple events that will be sent together
    /// in a federation transaction.
    pub async fn sign_outgoing_events(
        &self,
        events: &mut [Event],
        key_id: Option<&str>,
    ) -> Result<(), EventSigningError> {
        let signing_key = key_id.unwrap_or(&self.default_key_id);

        debug!("Batch signing {} events with key {}", events.len(), signing_key);

        for event in events.iter_mut() {
            self.validate_event_for_signing(event)?;
            self.signing_engine.sign_event(event, signing_key).await?;
        }

        info!("Successfully batch signed {} events", events.len());
        Ok(())
    }

    /// Pre-compute content hash for an event without signing
    ///
    /// Useful for event preparation before the full signing process.
    pub fn calculate_event_hash(&self, event: &Event) -> Result<String, EventSigningError> {
        self.signing_engine.calculate_content_hash(event)
    }

    /// Calculate reference hash for room versions that require it
    ///
    /// Some room versions use reference hashes for event IDs or validation.
    pub fn calculate_reference_hash(&self, event: &Event, room_version: &str) -> Result<String, EventSigningError> {
        self.signing_engine.calculate_reference_hash(event, room_version)
    }

    /// Validate an event is ready for signing
    ///
    /// Checks that all required fields are present and properly formatted
    /// before attempting the signing process.
    fn validate_event_for_signing(&self, event: &Event) -> Result<(), EventSigningError> {
        // Check required fields are present
        if event.event_id.is_empty() {
            return Err(EventSigningError::InvalidFormat(
                "Event ID is required for signing".to_string(),
            ));
        }

        if event.room_id.is_empty() {
            return Err(EventSigningError::InvalidFormat(
                "Room ID is required for signing".to_string(),
            ));
        }

        if event.sender.is_empty() {
            return Err(EventSigningError::InvalidFormat(
                "Sender is required for signing".to_string(),
            ));
        }

        if event.event_type.is_empty() {
            return Err(EventSigningError::InvalidFormat(
                "Event type is required for signing".to_string(),
            ));
        }

        // Validate sender domain matches our homeserver for outgoing events
        if !event.sender.ends_with(&format!(":{}", self.homeserver_name)) {
            return Err(EventSigningError::InvalidFormat(format!(
                "Cannot sign event from {}: sender domain must match homeserver {}",
                event.sender, self.homeserver_name
            )));
        }

        // Check origin_server_ts is reasonable (within 1 hour of now)
        let now = chrono::Utc::now().timestamp_millis();
        let age = (now - event.origin_server_ts).abs();
        if age > 3600000 {
            // 1 hour in milliseconds
            warn!(
                "Event {} has timestamp {} which is {} ms from now",
                event.event_id, event.origin_server_ts, age
            );
        }

        // Ensure event doesn't already have signatures from our server
        if let Some(signatures) = &event.signatures
            && let Ok(sigs_value) = serde_json::to_value(signatures)
                && let Some(obj) = sigs_value.as_object()
                && obj.contains_key(&self.homeserver_name) {
                        return Err(EventSigningError::InvalidFormat(
                            "Event already has signature from this server".to_string(),
                        ));
                    }

        debug!("Event {} validated for signing", event.event_id);
        Ok(())
    }

    /// Get the default signing key ID for this server
    ///
    /// Returns the key ID that will be used for signing if no specific key is requested.
    pub fn get_default_key_id(&self) -> &str {
        &self.default_key_id
    }

    /// Get the underlying event signing engine
    ///
    /// Returns a reference to the EventSigningEngine for cryptographic operations.
    pub fn get_signing_engine(&self) -> &EventSigningEngine {
        &self.signing_engine
    }

    /// Get available signing keys for this server
    ///
    /// Returns list of key IDs that can be used for signing events.
    pub async fn get_available_signing_keys(&self) -> Result<Vec<String>, EventSigningError> {
        // Get active signing key IDs using key server repository
        let key_ids = self
            .key_server_repo
            .get_active_signing_key_ids(&self.homeserver_name)
            .await
            .map_err(EventSigningError::DatabaseError)?;

        debug!("Found {} active signing keys", key_ids.len());
        Ok(key_ids)
    }

    /// Generate a new signing key for this server
    ///
    /// Creates a new Ed25519 key pair and stores it in the database
    /// for use in signing outgoing events. Integrates with MatrixSessionService
    /// for secure key generation and storage.
    pub async fn generate_new_signing_key(
        &self,
        key_name: Option<&str>,
    ) -> Result<String, EventSigningError> {
        use base64::{Engine as _, engine::general_purpose};
        use ed25519_dalek::{SigningKey as Ed25519SigningKey, VerifyingKey};
        

        let db = &self.signing_engine.db;
        
        // Validate database connection before key generation
        let version_info = match db.version().await {
            Ok(version) => version.to_string(),
            Err(_) => "unknown".to_string(),
        };
        debug!("Generating signing key with database connection to: {}", version_info);

        // Generate Ed25519 key pair using cryptographically secure random number generator
        let mut secret_bytes = [0u8; 32];
        getrandom::fill(&mut secret_bytes).expect("Failed to generate random bytes");
        let signing_key = Ed25519SigningKey::from_bytes(&secret_bytes);
        let verifying_key: VerifyingKey = (&signing_key).into();

        // Create key ID with base64-encoded key prefix for uniqueness
        let key_id = format!(
            "ed25519:{}",
            key_name.unwrap_or(&general_purpose::STANDARD.encode(&verifying_key.to_bytes()[..8]))
        );

        // Encode keys for database storage
        let private_key_b64 = general_purpose::STANDARD.encode(signing_key.to_bytes());
        let public_key_b64 = general_purpose::STANDARD.encode(verifying_key.to_bytes());

        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::days(365); // 1 year validity

        // Create SigningKey struct for repository storage
        let signing_key_entity = SigningKey {
            key_id: key_id.clone(),
            server_name: self.homeserver_name.clone(),
            signing_key: private_key_b64,
            verify_key: public_key_b64,
            created_at: now,
            expires_at: Some(expires_at),
        };

        // Store the new signing key using key server repository
        self.key_server_repo
            .store_signing_key(&self.homeserver_name, &key_id, &signing_key_entity)
            .await
            .map_err(EventSigningError::DatabaseError)?;

        info!(
            "Generated and stored new signing key: {} for server {} (expires: {})",
            key_id, self.homeserver_name, expires_at
        );

        Ok(key_id)
    }

    /// Create a redacted copy of an event
    ///
    /// Useful for debugging or creating canonical representations
    /// of events according to the Matrix redaction algorithm.
    pub fn redact_event(&self, event: &Event, room_version: &str) -> Result<serde_json::Value, EventSigningError> {
        self.signing_engine.redact_event(event, room_version)
    }



    /// Generate a server authentication token for federation requests
    ///
    /// Creates a JWT token for server-to-server authentication that can be used
    /// alongside X-Matrix signatures for enhanced federation security.
    ///
    /// # Arguments
    /// * `destination` - The destination server name
    ///
    /// # Returns
    /// * `Ok(String)` - The generated JWT token
    /// * `Err(EventSigningError)` - If token generation fails
    pub fn generate_federation_token(
        &self,
        _destination: &str,
    ) -> Result<String, EventSigningError> {
        self.signing_engine.session_service.create_server_token(
            &self.homeserver_name,
            &self.default_key_id,
            3600  // expires_in (1 hour)
        ).map_err(|e| EventSigningError::InvalidRequest(format!("Token generation failed: {}", e)))
    }

    /// Sign a federation request with X-Matrix authorization
    ///
    /// This method adds the required X-Matrix authorization header to HTTP requests
    /// for federation API calls, following the Matrix Server-Server API specification.
    /// Also adds a server authentication token for enhanced security.
    ///
    /// # Arguments
    /// * `request_builder` - The reqwest RequestBuilder to sign
    /// * `method` - The HTTP method (GET, POST, PUT, etc.)
    /// * `uri` - The request URI path and query (e.g., "/_matrix/federation/v1/media/download/example.org/abc123")
    /// * `destination` - The destination server name
    /// * `content` - Optional JSON request body content
    ///
    /// # Returns
    /// * `Ok(RequestBuilder)` - The signed request builder
    /// * `Err(EventSigningError)` - If signing fails
    pub async fn sign_federation_request(
        &self,
        request_builder: reqwest::RequestBuilder,
        method: &str,
        uri: &str,
        destination: &str,
        content: Option<serde_json::Value>,
    ) -> Result<reqwest::RequestBuilder, EventSigningError> {
        // Log the federation request signing attempt
        debug!("Attempting to sign federation request to destination: {}", destination);

        // Validate destination server name format
        if destination.is_empty() || !destination.contains('.') {
            return Err(EventSigningError::InvalidDestination(destination.to_string()));
        }

        // Generate server authentication token for enhanced security
        let server_token = self.generate_federation_token(destination)?;

        // Create federation request signer with complete Matrix specification implementation
        let signer = FederationRequestSigner::new(
            self.signing_engine.clone(),
            self.homeserver_name.clone(),
        );

        // Sign the request using complete Matrix JSON signing algorithm with provided parameters
        let signed_request = signer.sign_request_builder_with_content(
            request_builder, 
            method,
            uri,
            destination, 
            content
        ).await?;

        // Add server authentication token header
        let signed_request_with_token = signed_request.header("X-Matrix-Token", server_token);

        info!("Successfully signed federation request to destination: {}", destination);
        Ok(signed_request_with_token)
    }
}

/// Utility function to create EventSigner from application state
impl EventSigner {
    pub fn from_app_state(
        session_service: Arc<MatrixSessionService<Any>>,
        db: surrealdb::Surreal<surrealdb::engine::any::Any>,
        dns_resolver: Arc<MatrixDnsResolver>,
        homeserver_name: String,
    ) -> Result<Self, EventSigningError> {
        let default_key_id = "ed25519:auto".to_string();

        Self::new(session_service, db, dns_resolver, homeserver_name, default_key_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_event_validation() {
        let signer = create_test_signer().expect("Failed to create test signer");

        let mut event = Event {
            event_id: "$test:example.org".to_string(),
            room_id: "!room:example.org".to_string(),
            sender: "@user:example.org".to_string(),
            event_type: "m.room.message".to_string(),
            origin_server_ts: chrono::Utc::now().timestamp_millis(),
            content: matryx_entity::EventContent::Unknown(
                json!({"msgtype": "m.text", "body": "test"}),
            ),
            ..Default::default()
        };

        // Should pass validation for properly formed event
        assert!(signer.validate_event_for_signing(&event).is_ok());

        // Should fail validation for empty event_id
        event.event_id = "".to_string();
        assert!(signer.validate_event_for_signing(&event).is_err());
    }

    fn create_test_signer() -> Result<EventSigner, EventSigningError> {
        use matryx_surrealdb::test_utils::create_test_db;
        use std::sync::Arc;

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
        let well_known_client = Arc::new(crate::federation::well_known_client::WellKnownClient::new(http_client));
        let dns_resolver = Arc::new(crate::federation::dns_resolver::MatrixDnsResolver::new(well_known_client).expect("Failed to create DNS resolver"));

        EventSigner::new(
            session_service,
            test_db,
            dns_resolver,
            "test.example.org".to_string(),
            "ed25519:test".to_string(),
        )
    }
}
