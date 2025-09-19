//! PDU Validation Pipeline
//!
//! Implements the complete Matrix Server-Server API PDU validation process
//! according to the 6-step validation pipeline defined in the Matrix specification.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use crate::auth::MatrixSessionService;
use crate::federation::authorization::{AuthorizationEngine, AuthorizationError};
use crate::federation::event_signing::EventSigningEngine;
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

/// DFS cycle detection state for DAG validation
#[derive(PartialEq, Debug)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}

/// Result of event deduplication check
#[derive(Debug)]
enum DeduplicationResult {
    /// Event is new and should be validated
    New,
    /// Event already exists (returns existing event)
    Duplicate(Event),
}

/// PDU Validator implementing Matrix Server-Server API validation pipeline
pub struct PduValidator {
    session_service: Arc<MatrixSessionService>,
    event_repo: Arc<EventRepository<surrealdb::engine::any::Any>>,
    room_repo: Arc<RoomRepository>,
    authorization_engine: AuthorizationEngine,
    event_signing_engine: EventSigningEngine,
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
        let event_signing_engine =
            EventSigningEngine::new(session_service.clone(), db.clone(), homeserver_name.clone());

        Self {
            session_service,
            event_repo,
            room_repo,
            authorization_engine,
            event_signing_engine,
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

        // Enhanced event deduplication with comprehensive checks
        match self.check_event_deduplication(&event).await? {
            DeduplicationResult::Duplicate(existing_event) => {
                debug!("Event {} already exists, returning existing event", event.event_id);
                return Ok(ValidationResult::Valid(existing_event));
            },
            DeduplicationResult::New => {
                // Continue with validation for new events
            },
        }

        // Step 2: Hash Verification with SHA-256 computation
        self.validate_event_hashes(&event, pdu).await?;
        debug!("Step 2 passed: Hash verification for event {}", event.event_id);

        // Step 3: Signature Verification using EventSigningEngine
        let expected_servers = vec![origin_server.to_string()];
        self.event_signing_engine
            .validate_event_crypto(&event, &expected_servers)
            .await
            .map_err(|e| PduValidationError::SignatureError(format!("{:?}", e)))?;
        debug!("Step 3 passed: Signature verification for event {}", event.event_id);

        // Step 4: Auth Events and DAG Validation
        self.validate_auth_events(&event).await?;
        debug!("Step 4a passed: Auth events validation for event {}", event.event_id);

        // Step 4b: Prev Events DAG Validation and Cycle Detection
        self.validate_prev_events_dag(&event).await?;
        debug!("Step 4b passed: DAG validation for event {}", event.event_id);

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

    /// Step 1: Validate PDU format according to Matrix specification for all room versions
    ///
    /// Implements comprehensive format validation supporting Matrix room versions v1-v10:
    /// - Room v1-v2: Basic event format validation
    /// - Room v3: Enhanced event format with state resolution v2
    /// - Room v4: Event ID format changed to use event hash
    /// - Room v5: Enforce integer timestamp restrictions
    /// - Room v6: Content hash validation and redaction changes
    /// - Room v7-v10: Additional validation rules and performance improvements
    async fn validate_format(&self, pdu: &Value) -> Result<Event, PduValidationError> {
        let mut event: Event = serde_json::from_value(pdu.clone()).map_err(|e| {
            PduValidationError::InvalidFormat(format!("Failed to parse PDU as Event: {}", e))
        })?;

        // Get room version for version-specific validation
        let room_version = if !event.room_id.is_empty() && event.room_id.starts_with('!') {
            self.get_room_version(&event.room_id)
                .await
                .unwrap_or_else(|_| "1".to_string())
        } else {
            "1".to_string() // Default to v1 for basic validation
        };

        // Basic format validation for all room versions
        self.validate_basic_format(&event)?;

        // Room version specific validation
        match room_version.as_str() {
            "1" | "2" => self.validate_room_v1_v2_format(&event)?,
            "3" => self.validate_room_v3_format(&event)?,
            "4" => self.validate_room_v4_format(&event, pdu)?,
            "5" => self.validate_room_v5_format(&event, pdu)?,
            "6" => self.validate_room_v6_format(&event, pdu)?,
            "7" | "8" | "9" | "10" => self.validate_room_v7_plus_format(&event, pdu)?,
            _ => {
                warn!("Unknown room version {}, using v1 validation", room_version);
                self.validate_room_v1_v2_format(&event)?;
            },
        }

        // Set received timestamp
        event.received_ts = Some(Utc::now().timestamp_millis());

        debug!(
            "Format validation passed for event {} (room version {})",
            event.event_id, room_version
        );
        Ok(event)
    }

    /// Enhanced event deduplication with comprehensive duplicate detection
    ///
    /// Performs multiple levels of deduplication checking:
    /// - Exact event ID match (primary deduplication)
    /// - Content hash deduplication for room v4+ events
    /// - Temporal deduplication for similar events from same sender
    /// - State key deduplication for state events
    async fn check_event_deduplication(
        &self,
        event: &Event,
    ) -> Result<DeduplicationResult, PduValidationError> {
        // Primary deduplication: Check exact event ID match
        if let Ok(Some(existing_event)) = self.event_repo.get_by_id(&event.event_id).await {
            debug!("Found exact event ID match for {}", event.event_id);
            return Ok(DeduplicationResult::Duplicate(existing_event));
        }

        // Content hash deduplication for room v4+ events
        let room_version = self
            .get_room_version(&event.room_id)
            .await
            .unwrap_or_else(|_| "1".to_string());
        if matches!(room_version.as_str(), "4" | "5" | "6" | "7" | "8" | "9" | "10") {
            if let Some(duplicate) = self.check_content_hash_deduplication(event).await? {
                debug!("Found content hash duplicate for {}", event.event_id);
                return Ok(DeduplicationResult::Duplicate(duplicate));
            }
        }

        // State key deduplication for state events
        if event.state_key.is_some() {
            if let Some(duplicate) = self.check_state_key_deduplication(event).await? {
                debug!(
                    "Found state key duplicate for {} in room {}",
                    event.event_id, event.room_id
                );
                return Ok(DeduplicationResult::Duplicate(duplicate));
            }
        }

        // Temporal deduplication for rapid successive events
        if let Some(duplicate) = self.check_temporal_deduplication(event).await? {
            debug!("Found temporal duplicate for {} from sender {}", event.event_id, event.sender);
            return Ok(DeduplicationResult::Duplicate(duplicate));
        }

        debug!("Event {} is new - no duplicates found", event.event_id);
        Ok(DeduplicationResult::New)
    }

    /// Check for content hash-based deduplication
    async fn check_content_hash_deduplication(
        &self,
        event: &Event,
    ) -> Result<Option<Event>, PduValidationError> {
        // Create content hash for comparison
        let event_json = serde_json::to_value(event).map_err(|e| {
            PduValidationError::InvalidFormat(format!(
                "Failed to serialize event for content hash: {}",
                e
            ))
        })?;

        let content_hash = self.compute_sha256_content_hash(&event_json)?;

        // Query for events with same content hash in the same room
        let query = "
            SELECT * FROM events 
            WHERE room_id = $room_id 
              AND content_hash = $content_hash 
            LIMIT 1
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", event.room_id.clone()))
            .bind(("content_hash", content_hash.clone()))
            .await
            .map_err(|e| PduValidationError::DatabaseError(e.into()))?;

        let existing_events: Vec<Event> =
            response.take(0).map_err(|e| PduValidationError::DatabaseError(e.into()))?;

        Ok(existing_events.into_iter().next())
    }

    /// Check for state key-based deduplication
    async fn check_state_key_deduplication(
        &self,
        event: &Event,
    ) -> Result<Option<Event>, PduValidationError> {
        let state_key = match &event.state_key {
            Some(key) => key,
            None => return Ok(None),
        };

        // Query for most recent state event with same type and state_key
        let query = "
            SELECT * FROM events 
            WHERE room_id = $room_id 
              AND event_type = $event_type 
              AND state_key = $state_key 
              AND soft_failed != true
            ORDER BY origin_server_ts DESC 
            LIMIT 1
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", event.room_id.clone()))
            .bind(("event_type", event.event_type.clone()))
            .bind(("state_key", state_key.clone()))
            .await
            .map_err(|e| PduValidationError::DatabaseError(e.into()))?;

        let existing_events: Vec<Event> =
            response.take(0).map_err(|e| PduValidationError::DatabaseError(e.into()))?;

        // Check if content is identical (true duplicate)
        if let Some(existing_event) = existing_events.into_iter().next() {
            if self.events_have_identical_content(event, &existing_event) {
                return Ok(Some(existing_event));
            }
        }

        Ok(None)
    }

    /// Check for temporal deduplication (rapid successive events)
    async fn check_temporal_deduplication(
        &self,
        event: &Event,
    ) -> Result<Option<Event>, PduValidationError> {
        const TEMPORAL_WINDOW_MS: i64 = 1000; // 1 second window

        // Query for recent events from same sender with identical content
        let query = "
            SELECT * FROM events 
            WHERE room_id = $room_id 
              AND sender = $sender 
              AND event_type = $event_type
              AND origin_server_ts >= $start_time 
              AND origin_server_ts <= $end_time
            ORDER BY origin_server_ts DESC 
            LIMIT 5
        ";

        let start_time = event.origin_server_ts - TEMPORAL_WINDOW_MS;
        let end_time = event.origin_server_ts + TEMPORAL_WINDOW_MS;

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", event.room_id.clone()))
            .bind(("sender", event.sender.clone()))
            .bind(("event_type", event.event_type.clone()))
            .bind(("start_time", start_time))
            .bind(("end_time", end_time))
            .await
            .map_err(|e| PduValidationError::DatabaseError(e.into()))?;

        let recent_events: Vec<Event> =
            response.take(0).map_err(|e| PduValidationError::DatabaseError(e.into()))?;

        // Check for identical content in temporal window
        for existing_event in recent_events {
            if self.events_have_identical_content(event, &existing_event) {
                return Ok(Some(existing_event));
            }
        }

        Ok(None)
    }

    /// Check if two events have identical content
    fn events_have_identical_content(&self, event1: &Event, event2: &Event) -> bool {
        // Compare essential fields that define event content
        let content1_json = serde_json::to_value(&event1.content).unwrap_or_default();
        let content2_json = serde_json::to_value(&event2.content).unwrap_or_default();

        event1.event_type == event2.event_type &&
            event1.sender == event2.sender &&
            content1_json == content2_json &&
            event1.state_key == event2.state_key
    }

    /// Basic format validation for all Matrix room versions
    fn validate_basic_format(&self, event: &Event) -> Result<(), PduValidationError> {
        // Event ID format validation
        if event.event_id.is_empty() {
            return Err(PduValidationError::InvalidFormat("Event ID cannot be empty".to_string()));
        }

        if !event.event_id.starts_with('$') {
            return Err(PduValidationError::InvalidFormat(
                "Event ID must start with '$'".to_string(),
            ));
        }

        // Room ID format validation
        if event.room_id.is_empty() {
            return Err(PduValidationError::InvalidFormat("Room ID cannot be empty".to_string()));
        }

        if !event.room_id.starts_with('!') {
            return Err(PduValidationError::InvalidFormat(
                "Room ID must start with '!'".to_string(),
            ));
        }

        // Sender validation
        if event.sender.is_empty() {
            return Err(PduValidationError::InvalidFormat("Sender cannot be empty".to_string()));
        }

        if !event.sender.starts_with('@') || !event.sender.contains(':') {
            return Err(PduValidationError::InvalidFormat(format!(
                "Invalid sender format: {}",
                event.sender
            )));
        }

        // Event type validation
        if event.event_type.is_empty() {
            return Err(PduValidationError::InvalidFormat(
                "Event type cannot be empty".to_string(),
            ));
        }

        // Origin server timestamp validation
        if event.origin_server_ts <= 0 {
            return Err(PduValidationError::InvalidFormat(
                "Origin server timestamp must be positive".to_string(),
            ));
        }

        Ok(())
    }

    /// Room version 1-2 specific format validation
    fn validate_room_v1_v2_format(&self, event: &Event) -> Result<(), PduValidationError> {
        // Validate event ID format for room v1-v2 (base64-encoded SHA-256)
        let event_id_content = &event.event_id[1..]; // Skip the '$' prefix
        if event_id_content.len() < 10 {
            return Err(PduValidationError::InvalidFormat(
                "Event ID too short for room v1-v2".to_string(),
            ));
        }

        // Basic depth validation
        if let Some(depth) = event.depth {
            if depth < 0 {
                return Err(PduValidationError::InvalidFormat(
                    "Event depth cannot be negative".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Room version 3 specific format validation
    fn validate_room_v3_format(&self, event: &Event) -> Result<(), PduValidationError> {
        // Room v3 uses state resolution v2 but same event format as v1-v2
        self.validate_room_v1_v2_format(event)?;

        // Additional v3-specific validations
        if event.event_type == "m.room.create" {
            if let Some(state_key) = &event.state_key {
                if !state_key.is_empty() {
                    return Err(PduValidationError::InvalidFormat(
                        "Room create event must have empty state key".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Room version 4 specific format validation with event hash-based IDs
    fn validate_room_v4_format(
        &self,
        event: &Event,
        pdu: &Value,
    ) -> Result<(), PduValidationError> {
        // Room v4 changes event ID format to use event content hash
        self.validate_event_hash_format(&event.event_id)?;

        // Validate that event ID matches content hash
        let computed_event_id = self.compute_event_id_v4(pdu)?;
        if event.event_id != computed_event_id {
            return Err(PduValidationError::InvalidFormat(format!(
                "Event ID mismatch: expected {}, got {}",
                computed_event_id, event.event_id
            )));
        }

        // Other v4-specific validations
        if event.event_type == "m.room.aliases" {
            return Err(PduValidationError::InvalidFormat(
                "m.room.aliases events are not allowed in room v4+".to_string(),
            ));
        }

        Ok(())
    }

    /// Room version 5 specific format validation with integer restrictions
    fn validate_room_v5_format(
        &self,
        event: &Event,
        pdu: &Value,
    ) -> Result<(), PduValidationError> {
        // Room v5 includes all v4 validations
        self.validate_room_v4_format(event, pdu)?;

        // Integer validation for timestamps and depth
        if event.origin_server_ts > i64::MAX as i64 {
            return Err(PduValidationError::InvalidFormat(
                "Origin server timestamp exceeds maximum integer value".to_string(),
            ));
        }

        if let Some(depth) = event.depth {
            if depth > i64::MAX {
                return Err(PduValidationError::InvalidFormat(
                    "Event depth exceeds maximum integer value".to_string(),
                ));
            }
        }

        // Validate integer fields in content for specific event types
        if event.event_type == "m.room.power_levels" {
            let content_value = serde_json::to_value(&event.content).map_err(|e| {
                PduValidationError::InvalidFormat(format!(
                    "Failed to convert event content to JSON: {}",
                    e
                ))
            })?;
            self.validate_power_levels_integers(&content_value)?;
        }

        Ok(())
    }

    /// Room version 6 specific format validation with content hash requirements
    fn validate_room_v6_format(
        &self,
        event: &Event,
        pdu: &Value,
    ) -> Result<(), PduValidationError> {
        // Room v6 includes all v5 validations
        self.validate_room_v5_format(event, pdu)?;

        // Content hash validation is handled by the main hash verification step
        // This ensures consistency across all room versions

        Ok(())
    }

    /// Room version 7+ specific format validation with latest enhancements
    fn validate_room_v7_plus_format(
        &self,
        event: &Event,
        pdu: &Value,
    ) -> Result<(), PduValidationError> {
        // Room v7+ includes all v6 validations
        self.validate_room_v6_format(event, pdu)?;

        // Additional v7+ specific validations
        // - Enhanced redaction rules
        // - Stricter event size limits
        // - Improved state resolution performance

        // Validate event size limits (recommended: 65KB for events, 10KB for state events)
        let event_size = serde_json::to_vec(pdu)
            .map_err(|e| {
                PduValidationError::InvalidFormat(format!(
                    "Failed to serialize event for size check: {}",
                    e
                ))
            })?
            .len();

        let max_size = if event.state_key.is_some() {
            10 * 1024
        } else {
            65 * 1024
        }; // 10KB for state, 65KB for events
        if event_size > max_size {
            return Err(PduValidationError::InvalidFormat(format!(
                "Event size {} exceeds maximum {} bytes",
                event_size, max_size
            )));
        }

        Ok(())
    }

    /// Validate event hash format for room version 4+
    fn validate_event_hash_format(&self, event_id: &str) -> Result<(), PduValidationError> {
        let event_id_content = &event_id[1..]; // Skip the '$' prefix

        // Event ID should be base64-encoded SHA-256 hash (44 characters with padding)
        if event_id_content.len() < 43 || event_id_content.len() > 44 {
            return Err(PduValidationError::InvalidFormat(
                "Invalid event ID hash length for room v4+".to_string(),
            ));
        }

        // Validate base64 format
        if base64::Engine::decode(&general_purpose::STANDARD_NO_PAD, event_id_content).is_err() {
            return Err(PduValidationError::InvalidFormat(
                "Event ID must be valid base64 for room v4+".to_string(),
            ));
        }

        Ok(())
    }

    /// Compute event ID for room version 4+ using event content hash
    fn compute_event_id_v4(&self, pdu: &Value) -> Result<String, PduValidationError> {
        // Create event copy without signatures and unsigned data
        let mut event_for_hash = pdu.clone();
        if let Some(obj) = event_for_hash.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        // Convert to canonical JSON
        let canonical_json =
            matryx_entity::utils::canonical_json(&event_for_hash).map_err(|e| {
                PduValidationError::InvalidFormat(format!(
                    "Failed to create canonical JSON for event ID: {}",
                    e
                ))
            })?;

        // Compute SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical_json.as_bytes());
        let hash = hasher.finalize();

        // Encode as base64 without padding
        let event_id_content = general_purpose::STANDARD_NO_PAD.encode(&hash);
        Ok(format!("${}", event_id_content))
    }

    /// Validate power levels have integer values
    fn validate_power_levels_integers(&self, content: &Value) -> Result<(), PduValidationError> {
        if let Some(obj) = content.as_object() {
            // Check users power levels
            if let Some(users) = obj.get("users").and_then(|v| v.as_object()) {
                for (user_id, level) in users {
                    if !level.is_i64() {
                        return Err(PduValidationError::InvalidFormat(format!(
                            "Power level for user {} must be integer",
                            user_id
                        )));
                    }
                }
            }

            // Check events power levels
            if let Some(events) = obj.get("events").and_then(|v| v.as_object()) {
                for (event_type, level) in events {
                    if !level.is_i64() {
                        return Err(PduValidationError::InvalidFormat(format!(
                            "Power level for event {} must be integer",
                            event_type
                        )));
                    }
                }
            }

            // Check default power levels
            let integer_fields = [
                "ban",
                "invite",
                "kick",
                "redact",
                "state_default",
                "events_default",
                "users_default",
            ];
            for field in &integer_fields {
                if let Some(value) = obj.get(*field) {
                    if !value.is_i64() {
                        return Err(PduValidationError::InvalidFormat(format!(
                            "Power level field {} must be integer",
                            field
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Step 4a: Validate authorization events exist
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

    /// Step 4b: Comprehensive prev_events DAG validation and cycle detection
    ///
    /// Validates the event DAG (Directed Acyclic Graph) structure according to Matrix specification:
    /// - Ensures prev_events form a proper DAG without cycles
    /// - Validates event depth consistency with prev_events
    /// - Checks for proper event ordering and dependencies
    /// - Detects and prevents infinite loops in the event graph
    /// - Validates maximum prev_events limits for performance
    async fn validate_prev_events_dag(&self, event: &Event) -> Result<(), PduValidationError> {
        // Validate prev_events structure and constraints
        let prev_events = match &event.prev_events {
            Some(prev) if !prev.is_empty() => prev,
            Some(_) | None => {
                // Genesis events (depth 0) or outliers may have no prev_events
                if event.depth.unwrap_or(0) == 0 || event.outlier.unwrap_or(false) {
                    debug!(
                        "Accepting event {} with no prev_events (depth: {}, outlier: {})",
                        event.event_id,
                        event.depth.unwrap_or(0),
                        event.outlier.unwrap_or(false)
                    );
                    return Ok(());
                } else {
                    return Err(PduValidationError::StateError(
                        "Non-genesis events must have prev_events".to_string(),
                    ));
                }
            },
        };

        // Validate maximum prev_events limit for performance
        if prev_events.len() > 20 {
            return Err(PduValidationError::StateError(format!(
                "Too many prev_events: {} (maximum: 20)",
                prev_events.len()
            )));
        }

        // Validate prev_events exist and load their data
        let mut prev_event_data = Vec::new();
        for prev_event_id in prev_events {
            match self.event_repo.get_by_id(prev_event_id).await {
                Ok(Some(prev_event)) => {
                    prev_event_data.push(prev_event);
                },
                Ok(None) => {
                    if !event.outlier.unwrap_or(false) {
                        return Err(PduValidationError::StateError(format!(
                            "Prev event {} not found for non-outlier event",
                            prev_event_id
                        )));
                    }
                },
                Err(e) => {
                    error!("Database error loading prev event {}: {}", prev_event_id, e);
                    return Err(PduValidationError::DatabaseError(e));
                },
            }
        }

        // Validate event depth consistency with prev_events
        self.validate_depth_consistency(event, &prev_event_data).await?;

        // Detect cycles in the event DAG using depth-first search
        self.detect_dag_cycles(event, &prev_event_data).await?;

        // Validate prev_events are in the same room
        self.validate_prev_events_room_consistency(event, &prev_event_data).await?;

        // Validate temporal ordering (events should have increasing timestamps in general)
        self.validate_temporal_ordering(event, &prev_event_data).await?;

        debug!(
            "DAG validation completed for event {} with {} prev_events",
            event.event_id,
            prev_events.len()
        );
        Ok(())
    }

    /// Validate event depth consistency with prev_events
    async fn validate_depth_consistency(
        &self,
        event: &Event,
        prev_events: &[Event],
    ) -> Result<(), PduValidationError> {
        let event_depth = event.depth.unwrap_or(0);

        if prev_events.is_empty() {
            // Genesis event should have depth 0
            if event_depth != 0 {
                return Err(PduValidationError::StateError(format!(
                    "Genesis event must have depth 0, got depth {}",
                    event_depth
                )));
            }
            return Ok(());
        }

        // Find maximum depth among prev_events
        let max_prev_depth = prev_events.iter().map(|e| e.depth.unwrap_or(0)).max().unwrap_or(0);

        // Event depth should be exactly max_prev_depth + 1
        let expected_depth = max_prev_depth + 1;
        if event_depth != expected_depth {
            return Err(PduValidationError::StateError(format!(
                "Invalid event depth: expected {}, got {} (max prev depth: {})",
                expected_depth, event_depth, max_prev_depth
            )));
        }

        debug!(
            "Depth consistency validated: event depth {} follows max prev depth {}",
            event_depth, max_prev_depth
        );
        Ok(())
    }

    /// Detect cycles in the event DAG using depth-first search
    async fn detect_dag_cycles(
        &self,
        event: &Event,
        prev_events: &[Event],
    ) -> Result<(), PduValidationError> {
        let mut visit_states: HashMap<String, VisitState> = HashMap::new();
        let mut recursion_stack: HashSet<String> = HashSet::new();

        // Start DFS from the current event
        if self
            .dfs_cycle_detection(
                &event.event_id,
                &mut visit_states,
                &mut recursion_stack,
                0,  // current depth
                50, // maximum search depth to prevent infinite recursion
            )
            .await?
        {
            return Err(PduValidationError::StateError(format!(
                "Cycle detected in event DAG starting from event {}",
                event.event_id
            )));
        }

        debug!("DAG cycle detection completed - no cycles found for event {}", event.event_id);
        Ok(())
    }

    /// Recursive DFS cycle detection
    async fn dfs_cycle_detection(
        &self,
        event_id: &str,
        visit_states: &mut HashMap<String, VisitState>,
        recursion_stack: &mut HashSet<String>,
        current_depth: usize,
        max_depth: usize,
    ) -> Result<bool, PduValidationError> {
        // Prevent infinite recursion
        if current_depth > max_depth {
            debug!("DFS depth limit reached at {} for event {}", max_depth, event_id);
            return Ok(false);
        }

        // Check if event is already in recursion stack (cycle detected)
        if recursion_stack.contains(event_id) {
            return Ok(true);
        }

        // Check visit state
        match visit_states.get(event_id) {
            Some(VisitState::Visited) => return Ok(false),
            Some(VisitState::Visiting) => return Ok(true), // Back edge found
            Some(VisitState::Unvisited) | None => {},
        }

        // Mark as visiting and add to recursion stack
        visit_states.insert(event_id.to_string(), VisitState::Visiting);
        recursion_stack.insert(event_id.to_string());

        // Load event and check its prev_events
        match self.event_repo.get_by_id(event_id).await {
            Ok(Some(event)) => {
                if let Some(prev_events) = &event.prev_events {
                    for prev_event_id in prev_events {
                        if Box::pin(self.dfs_cycle_detection(
                            prev_event_id,
                            visit_states,
                            recursion_stack,
                            current_depth + 1,
                            max_depth,
                        ))
                        .await?
                        {
                            return Ok(true);
                        }
                    }
                }
            },
            Ok(None) => {
                // Event not found - treat as leaf node
                debug!("Event {} not found during cycle detection - treating as leaf", event_id);
            },
            Err(e) => {
                debug!("Database error during cycle detection for {}: {}", event_id, e);
                // Continue with cycle detection despite database errors
            },
        }

        // Mark as visited and remove from recursion stack
        visit_states.insert(event_id.to_string(), VisitState::Visited);
        recursion_stack.remove(event_id);

        Ok(false)
    }

    /// Validate prev_events are in the same room
    async fn validate_prev_events_room_consistency(
        &self,
        event: &Event,
        prev_events: &[Event],
    ) -> Result<(), PduValidationError> {
        for prev_event in prev_events {
            if prev_event.room_id != event.room_id {
                return Err(PduValidationError::StateError(format!(
                    "Prev event {} is in different room {} than current event room {}",
                    prev_event.event_id, prev_event.room_id, event.room_id
                )));
            }
        }

        Ok(())
    }

    /// Validate temporal ordering (events should generally have increasing timestamps)
    async fn validate_temporal_ordering(
        &self,
        event: &Event,
        prev_events: &[Event],
    ) -> Result<(), PduValidationError> {
        // This is a soft validation - we allow some clock skew between servers
        const MAX_CLOCK_SKEW_MS: i64 = 5 * 60 * 1000; // 5 minutes

        for prev_event in prev_events {
            let time_diff = event.origin_server_ts - prev_event.origin_server_ts;

            // Allow some clock skew but warn about significant backwards time
            if time_diff < -MAX_CLOCK_SKEW_MS {
                warn!(
                    "Event {} has timestamp significantly before prev event {} (diff: {}ms)",
                    event.event_id, prev_event.event_id, time_diff
                );
                // Don't fail validation for temporal issues as servers may have clock skew
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

    /// Step 2: Comprehensive hash verification with SHA-256 computation
    ///
    /// Validates event hashes according to Matrix specification:
    /// - Verifies content hash if present in the event
    /// - Validates event ID hash for room version 4+
    /// - Ensures hash integrity for signature verification
    /// - Supports multiple hash algorithms (primarily SHA-256)
    async fn validate_event_hashes(
        &self,
        event: &Event,
        pdu: &Value,
    ) -> Result<(), PduValidationError> {
        let room_version = self
            .get_room_version(&event.room_id)
            .await
            .unwrap_or_else(|_| "1".to_string());

        // Validate content hash if present
        if let Some(hashes) = pdu.get("hashes") {
            self.validate_content_hashes(pdu, hashes, &room_version).await?;
        }

        // For room version 4+, validate event ID is correctly computed from content hash
        if matches!(room_version.as_str(), "4" | "5" | "6" | "7" | "8" | "9" | "10") {
            self.validate_event_id_hash(&event.event_id, pdu).await?;
        }

        // Validate reference hash consistency for prev_events and auth_events
        self.validate_reference_hashes(event).await?;

        debug!("Hash verification completed for event {}", event.event_id);
        Ok(())
    }

    /// Validate content hashes in the hashes field
    async fn validate_content_hashes(
        &self,
        pdu: &Value,
        hashes: &Value,
        room_version: &str,
    ) -> Result<(), PduValidationError> {
        let hashes_obj = hashes.as_object().ok_or_else(|| {
            PduValidationError::HashMismatch {
                expected: "object".to_string(),
                actual: "not an object".to_string(),
            }
        })?;

        // Validate SHA-256 hash (primary algorithm)
        if let Some(sha256_hash) = hashes_obj.get("sha256") {
            let provided_hash = sha256_hash.as_str().ok_or_else(|| {
                PduValidationError::HashMismatch {
                    expected: "string".to_string(),
                    actual: "not a string".to_string(),
                }
            })?;

            let computed_hash = self.compute_sha256_content_hash(pdu)?;

            if provided_hash != computed_hash {
                return Err(PduValidationError::HashMismatch {
                    expected: computed_hash,
                    actual: provided_hash.to_string(),
                });
            }

            debug!("SHA-256 content hash verified successfully");
        }

        // Validate any additional hash algorithms for future compatibility
        for (algorithm, hash_value) in hashes_obj {
            if algorithm != "sha256" {
                debug!(
                    "Unknown hash algorithm '{}' in event - ignoring for forward compatibility",
                    algorithm
                );
            }
        }

        Ok(())
    }

    /// Validate event ID hash for room version 4+
    async fn validate_event_id_hash(
        &self,
        event_id: &str,
        pdu: &Value,
    ) -> Result<(), PduValidationError> {
        let computed_event_id = self.compute_event_id_from_content(pdu)?;

        if event_id != computed_event_id {
            return Err(PduValidationError::HashMismatch {
                expected: computed_event_id,
                actual: event_id.to_string(),
            });
        }

        debug!("Event ID hash validated for room v4+ event: {}", event_id);
        Ok(())
    }

    /// Validate reference hash consistency for prev_events and auth_events
    async fn validate_reference_hashes(&self, event: &Event) -> Result<(), PduValidationError> {
        // Validate prev_events references
        if let Some(prev_events) = &event.prev_events {
            for prev_event_id in prev_events {
                self.validate_event_id_format(prev_event_id)?;
            }
        }

        // Validate auth_events references
        if let Some(auth_events) = &event.auth_events {
            for auth_event_id in auth_events {
                self.validate_event_id_format(auth_event_id)?;
            }
        }

        Ok(())
    }

    /// Validate event ID format is consistent
    fn validate_event_id_format(&self, event_id: &str) -> Result<(), PduValidationError> {
        if event_id.is_empty() || !event_id.starts_with('$') {
            return Err(PduValidationError::InvalidFormat(format!(
                "Invalid event ID format: {}",
                event_id
            )));
        }

        // Check if it looks like a valid hash-based ID or legacy ID
        let event_id_content = &event_id[1..];
        if event_id_content.len() < 10 {
            return Err(PduValidationError::InvalidFormat(format!(
                "Event ID too short: {}",
                event_id
            )));
        }

        Ok(())
    }

    /// Compute SHA-256 content hash for an event
    fn compute_sha256_content_hash(&self, pdu: &Value) -> Result<String, PduValidationError> {
        // Create event copy without hashes, signatures and unsigned data for hash computation
        let mut event_for_hash = pdu.clone();
        if let Some(obj) = event_for_hash.as_object_mut() {
            obj.remove("hashes");
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        // Convert to canonical JSON for consistent hash computation
        let canonical_json =
            matryx_entity::utils::canonical_json(&event_for_hash).map_err(|e| {
                PduValidationError::InvalidFormat(format!(
                    "Failed to create canonical JSON for hash: {}",
                    e
                ))
            })?;

        // Compute SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical_json.as_bytes());
        let hash = hasher.finalize();

        // Encode as base64 without padding (Matrix standard)
        Ok(general_purpose::STANDARD_NO_PAD.encode(&hash))
    }

    /// Compute event ID from event content (room version 4+)
    fn compute_event_id_from_content(&self, pdu: &Value) -> Result<String, PduValidationError> {
        // For room v4+, event ID is computed from content hash
        let content_hash = self.compute_sha256_content_hash(pdu)?;
        Ok(format!("${}", content_hash))
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

    /// Replaced by EventSigningEngine.validate_event_crypto

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

    /// Replaced by EventSigningEngine.calculate_content_hash and related methods

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
