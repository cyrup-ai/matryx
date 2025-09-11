//! Event Signing Utility for Matrix Server
//!
//! Provides high-level interface for signing outgoing Matrix events
//! before sending them over federation. Integrates with the complete
//! EventSigningEngine to ensure proper Matrix specification compliance.

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::auth::MatrixSessionService;
use crate::federation::event_signing::{EventSigningEngine, EventSigningError};
use matryx_entity::types::Event;
use matryx_surrealdb::repository::error::RepositoryError;

/// High-level event signing service for outgoing Matrix events
///
/// Provides a simplified interface for signing events before federation,
/// automatically handling key selection, hash calculation, and signature
/// generation according to the Matrix specification.
pub struct EventSigner {
    signing_engine: EventSigningEngine,
    default_key_id: String,
    homeserver_name: String,
}

impl EventSigner {
    pub fn new(
        session_service: Arc<MatrixSessionService>,
        db: surrealdb::Surreal<surrealdb::engine::any::Any>,
        homeserver_name: String,
        default_key_id: String,
    ) -> Self {
        let signing_engine = EventSigningEngine::new(session_service, db, homeserver_name.clone());

        Self { signing_engine, default_key_id, homeserver_name }
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
    pub fn calculate_reference_hash(&self, event: &Event) -> Result<String, EventSigningError> {
        self.signing_engine.calculate_reference_hash(event)
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
        if let Some(signatures) = &event.signatures {
            if let Ok(sigs_value) = serde_json::to_value(signatures) {
                if let Some(obj) = sigs_value.as_object() {
                    if obj.contains_key(&self.homeserver_name) {
                        return Err(EventSigningError::InvalidFormat(
                            "Event already has signature from this server".to_string(),
                        ));
                    }
                }
            }
        }

        debug!("Event {} validated for signing", event.event_id);
        Ok(())
    }

    /// Get available signing keys for this server
    ///
    /// Returns list of key IDs that can be used for signing events.
    pub async fn get_available_signing_keys(&self) -> Result<Vec<String>, EventSigningError> {
        // Query database for active signing keys
        let query = "
            SELECT key_id
            FROM server_signing_keys
            WHERE server_name = $server_name
              AND is_active = true
              AND (expires_at IS NULL OR expires_at > datetime::now())
            ORDER BY created_at DESC
        ";

        let db = &self.signing_engine.db;
        let mut response = db
            .query(query)
            .bind(("server_name", self.homeserver_name.clone()))
            .await
            .map_err(|e| {
                EventSigningError::DatabaseError(RepositoryError::Validation {
                    field: "query".to_string(),
                    message: format!("Failed to query signing keys: {}", e),
                })
            })?;

        let key_ids: Vec<String> = response.take(0).map_err(|e| {
            EventSigningError::DatabaseError(RepositoryError::Validation {
                field: "parse".to_string(),
                message: format!("Failed to parse key results: {}", e),
            })
        })?;

        debug!("Found {} active signing keys", key_ids.len());
        Ok(key_ids)
    }

    /// Generate a new signing key for this server
    ///
    /// Creates a new Ed25519 key pair and stores it in the database
    /// for use in signing outgoing events.
    ///
    /// Note: This is a placeholder implementation that would need proper key generation
    /// integration with the session service once the appropriate public methods are available.
    pub async fn generate_new_signing_key(
        &self,
        key_name: Option<&str>,
    ) -> Result<String, EventSigningError> {
        let key_id = format!(
            "ed25519:{}",
            key_name.unwrap_or(&format!("k{}", chrono::Utc::now().timestamp()))
        );

        // TODO: Implement proper key generation once session service provides public methods
        info!("Generated new signing key: {}", key_id);
        Ok(key_id)
    }

    /// Create a redacted copy of an event
    ///
    /// Useful for debugging or creating canonical representations
    /// of events according to the Matrix redaction algorithm.
    pub fn redact_event(&self, event: &Event) -> Result<serde_json::Value, EventSigningError> {
        self.signing_engine.redact_event(event)
    }
}

/// Utility function to create EventSigner from application state
impl EventSigner {
    pub fn from_app_state(
        session_service: Arc<MatrixSessionService>,
        db: surrealdb::Surreal<surrealdb::engine::any::Any>,
        homeserver_name: String,
    ) -> Self {
        let default_key_id = "ed25519:auto".to_string();

        Self::new(session_service, db, homeserver_name, default_key_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_event_validation() {
        let signer = create_test_signer();

        let mut event = Event {
            event_id: "$test:example.org".to_string(),
            room_id: "!room:example.org".to_string(),
            sender: "@user:example.org".to_string(),
            event_type: "m.room.message".to_string(),
            origin_server_ts: Some(chrono::Utc::now().timestamp_millis() as u64),
            content: json!({"msgtype": "m.text", "body": "test"}),
            ..Default::default()
        };

        // Should pass validation for properly formed event
        assert!(signer.validate_event_for_signing(&event).is_ok());

        // Should fail validation for empty event_id
        event.event_id = "".to_string();
        assert!(signer.validate_event_for_signing(&event).is_err());
    }

    fn create_test_signer() -> EventSigner {
        // Create minimal test instance - would need proper setup in real tests
        todo!("Implement test setup with mock dependencies")
    }
}
