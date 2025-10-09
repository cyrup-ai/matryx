use crate::repository::error::RepositoryError;
use crate::repository::power_levels::{PowerLevelsRepository, PowerLevelAction};
use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use ed25519_dalek::{SigningKey, Signature, VerifyingKey, Signer, Verifier};
use futures::{Stream, StreamExt};
use matryx_entity::types::{Event, EventContent, MembershipState};
use matryx_entity::utils::canonical_json::canonical_json_for_signing;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EventValidationResult {
    pub valid: bool,
    pub reason: Option<String>,
    pub error_code: Option<String>,
}

/// Parameters for creating federation membership events
#[derive(Debug, Clone)]
pub struct FederationMembershipParams<'a> {
    pub room_id: &'a str,
    pub sender: &'a str,
    pub target: &'a str,
    pub membership: matryx_entity::types::MembershipState,
    pub reason: Option<&'a str>,
    pub depth: i64,
    pub prev_events: &'a [String],
    pub auth_events: &'a [String],
    pub homeserver_name: &'a str,
}

#[derive(Debug, Clone)]
pub struct SignatureValidation {
    pub valid: bool,
    pub verified_signatures: HashMap<String, HashMap<String, bool>>,
    pub missing_signatures: Vec<String>,
}

// Additional response types for context and relations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventContext {
    pub events_before: Vec<Event>,
    pub event: Option<Event>,
    pub events_after: Vec<Event>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub state: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventReport {
    pub event_id: String,
    pub reporter_id: String,
    pub reason: String,
    pub score: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct EventRepository {
    db: Surreal<Any>,
}

impl EventRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, event: &Event) -> Result<Event, RepositoryError> {
        let event_clone = event.clone();
        let created: Option<Event> =
            self.db.create(("event", &event.event_id)).content(event_clone).await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create event"))
        })
    }

    pub async fn get_by_id(&self, event_id: &str) -> Result<Option<Event>, RepositoryError> {
        let event: Option<Event> = self.db.select(("event", event_id)).await?;
        Ok(event)
    }

    pub async fn get_room_events(
        &self,
        room_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = match limit {
            Some(l) => {
                format!(
                    "SELECT * FROM event WHERE room_id = $room_id ORDER BY origin_server_ts DESC LIMIT {}",
                    l
                )
            },
            None => {
                "SELECT * FROM event WHERE room_id = $room_id ORDER BY origin_server_ts DESC"
                    .to_string()
            },
        };

        let room_id_owned = room_id.to_string();
        let events: Vec<Event> =
            self.db.query(&query).bind(("room_id", room_id_owned)).await?.take(0)?;
        Ok(events)
    }

    /// Get room events since a specific received_ts (for incremental sync)
    pub async fn get_room_events_since(
        &self,
        room_id: &str,
        since_ts: Option<i64>,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = match (since_ts, limit) {
            (Some(ts), Some(l)) => {
                format!(
                    "SELECT * FROM event WHERE room_id = $room_id AND received_ts > {} ORDER BY received_ts ASC LIMIT {}",
                    ts, l
                )
            },
            (Some(ts), None) => {
                format!(
                    "SELECT * FROM event WHERE room_id = $room_id AND received_ts > {} ORDER BY received_ts ASC",
                    ts
                )
            },
            (None, Some(l)) => {
                format!(
                    "SELECT * FROM event WHERE room_id = $room_id ORDER BY received_ts ASC LIMIT {}",
                    l
                )
            },
            (None, None) => {
                "SELECT * FROM event WHERE room_id = $room_id ORDER BY received_ts ASC".to_string()
            },
        };

        let room_id_owned = room_id.to_string();
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id_owned))
            .await?;

        let events: Vec<Event> = result.take(0)?;
        Ok(events)
    }

    pub async fn get_state_events(&self, room_id: &str) -> Result<Vec<Event>, RepositoryError> {
        let room_id_owned = room_id.to_string();
        let events: Vec<Event> = self
            .db
            .query("SELECT * FROM event WHERE room_id = $room_id AND state_key IS NOT NULL")
            .bind(("room_id", room_id_owned))
            .await?
            .take(0)?;
        Ok(events)
    }

    /// Get the room creation event (m.room.create) for a specific room
    /// This is essential for determining room version and authorization rules
    pub async fn get_room_create_event(
        &self,
        room_id: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        let room_id_owned = room_id.to_string();
        let events: Vec<Event> = self
            .db
            .query("SELECT * FROM event WHERE room_id = $room_id AND event_type = 'm.room.create' AND state_key = '' LIMIT 1")
            .bind(("room_id", room_id_owned))
            .await?
            .take(0)?;

        Ok(events.into_iter().next())
    }

    /// Subscribe to real-time room events using SurrealDB LiveQuery
    /// Returns a stream of notifications for new events in the specified room
    pub async fn subscribe_room_events(
        &self,
        room_id: &str,
    ) -> Result<impl Stream<Item = Result<Event, RepositoryError>>, RepositoryError> {
        // Create SurrealDB LiveQuery for events in specific room (message events only)
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM event WHERE room_id = $room_id AND state_key IS NULL")
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to event stream
        let event_stream = stream
            .stream::<surrealdb::Notification<Event>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Event, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted events, we still return the event data
                        // so consumers can handle deletion/redaction appropriately
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(event_stream)
    }

    /// Subscribe to real-time room state events using SurrealDB LiveQuery
    /// Returns a stream of notifications for state changes in the specified room
    pub async fn subscribe_room_state_events(
        &self,
        room_id: &str,
    ) -> Result<impl Stream<Item = Result<Event, RepositoryError>>, RepositoryError> {
        // Create SurrealDB LiveQuery for state events in specific room
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM event WHERE room_id = $room_id AND state_key IS NOT NULL")
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to event stream
        let event_stream = stream
            .stream::<surrealdb::Notification<Event>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Event, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted state events, return the data for proper state resolution
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(event_stream)
    }

    /// Subscribe to all events for a specific user across all rooms they have access to
    /// Returns a stream of notifications for events the user can see
    pub async fn subscribe_user_events(
        &self,
        user_id: &str,
    ) -> Result<impl Stream<Item = Result<Event, RepositoryError>>, RepositoryError> {
        // Create SurrealDB LiveQuery for events in rooms where user has membership
        let mut stream = self
            .db
            .query(
                r#"
                LIVE SELECT * FROM event
                WHERE room_id IN (
                    SELECT VALUE room_id FROM membership
                    WHERE user_id = $user_id AND membership IN ['join', 'invite']
                )
            "#,
            )
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to event stream
        let event_stream = stream
            .stream::<surrealdb::Notification<Event>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Event, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted events, return data for proper handling
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(event_stream)
    }

    /// Check if an event already exists (duplicate detection)
    pub async fn check_duplicate(&self, event_id: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT count() FROM event WHERE event_id = $event_id GROUP ALL";
        let mut result = self.db.query(query).bind(("event_id", event_id.to_string())).await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Validate event relationships (auth events, prev events, etc.)
    pub async fn validate_event_relationships(
        &self,
        event: &Event,
    ) -> Result<bool, RepositoryError> {
        // Check if auth events exist
        if let Some(auth_events) = &event.auth_events {
            for auth_event_id in auth_events {
                let exists = self.check_duplicate(auth_event_id).await?;
                if !exists {
                    return Ok(false);
                }
            }
        }

        // Check if prev events exist
        if let Some(prev_events) = &event.prev_events {
            for prev_event_id in prev_events {
                let exists = self.check_duplicate(prev_event_id).await?;
                if !exists {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Get event by ID (alias for get_by_id for consistency)
    pub async fn get_event_by_id(&self, event_id: &str) -> Result<Option<Event>, RepositoryError> {
        self.get_by_id(event_id).await
    }

    /// Create a join event for a user joining a room
    pub async fn create_join_event(
        &self,
        room_id: &str,
        user_id: &str,
        content: Value,
    ) -> Result<Event, RepositoryError> {
        let event_id = format!("${}", Uuid::new_v4());
        let now = Utc::now();

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: user_id.to_string(),
            event_type: "m.room.member".to_string(),
            content: EventContent::Unknown(content),
            state_key: Some(user_id.to_string()),
            origin_server_ts: now.timestamp_millis(),
            unsigned: None,
            prev_events: None,
            auth_events: None,
            depth: None,
            hashes: None,
            signatures: None,
            redacts: None,
            outlier: Some(false),
            received_ts: Some(now.timestamp_millis()),
            rejected_reason: None,
            soft_failed: Some(false),
        };

        self.create(&event).await
    }

    /// Create a membership event for room joining/leaving
    pub async fn create_membership_event(
        &self,
        room_id: &str,
        user_id: &str,
        membership: MembershipState,
    ) -> Result<Event, RepositoryError> {
        let membership_content = serde_json::json!({
            "membership": membership.to_string(),
            "displayname": null,
            "avatar_url": null
        });

        self.create_join_event(room_id, user_id, membership_content).await
    }

    /// Get room timeline events with optional limit
    pub async fn get_room_timeline(
        &self,
        room_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, RepositoryError> {
        self.get_room_events(room_id, limit).await
    }

    /// Create a room event (state or message)
    pub async fn create_room_event(
        &self,
        room_id: &str,
        event_type: &str,
        sender: &str,
        content: Value,
        state_key: Option<String>,
    ) -> Result<Event, RepositoryError> {
        let event_id = format!("${}", Uuid::new_v4());
        let now = Utc::now();

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: sender.to_string(),
            event_type: event_type.to_string(),
            content: EventContent::Unknown(content),
            state_key,
            origin_server_ts: now.timestamp_millis(),
            unsigned: None,
            prev_events: None,
            auth_events: None,
            depth: None,
            hashes: None,
            signatures: None,
            redacts: None,
            outlier: Some(false),
            received_ts: Some(now.timestamp_millis()),
            rejected_reason: None,
            soft_failed: Some(false),
        };

        self.create(&event).await
    }

    /// Send a message event to a room
    pub async fn send_message_event(
        &self,
        room_id: &str,
        sender: &str,
        content: Value,
        txn_id: Option<String>,
    ) -> Result<Event, RepositoryError> {
        // Check for transaction duplicate if txn_id provided
        if let Some(ref txn) = txn_id
            && self.check_transaction_duplicate(txn).await? {
                return Err(RepositoryError::Conflict {
                    message: "Transaction ID already used".to_string(),
                });
            }

        let event = self
            .create_room_event(room_id, "m.room.message", sender, content, None)
            .await?;

        // Store transaction ID if provided
        if let Some(txn) = txn_id {
            let query = "CREATE transaction SET txn_id = $txn_id, event_id = $event_id, created_at = $created_at";
            self.db
                .query(query)
                .bind(("txn_id", txn))
                .bind(("event_id", event.event_id.clone()))
                .bind(("created_at", Utc::now()))
                .await?;
        }

        Ok(event)
    }

    /// Create a membership change event
    pub async fn create_membership_change_event(
        &self,
        room_id: &str,
        user_id: &str,
        target_user: &str,
        membership: MembershipState,
        reason: Option<String>,
    ) -> Result<Event, RepositoryError> {
        let mut content = serde_json::json!({
            "membership": membership.to_string(),
            "displayname": null,
            "avatar_url": null
        });

        if let Some(reason) = reason {
            content["reason"] = serde_json::Value::String(reason);
        }

        self.create_room_event(
            room_id,
            "m.room.member",
            user_id,
            content,
            Some(target_user.to_string()),
        )
        .await
    }

    /// Validate an event for a room
    pub async fn validate_event_for_room(
        &self,
        room_id: &str,
        event: &Event,
    ) -> Result<bool, RepositoryError> {
        // Basic validation - check if room exists
        let query = "SELECT room_id FROM room WHERE room_id = $room_id LIMIT 1";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let rooms: Vec<serde_json::Value> = result.take(0)?;

        if rooms.is_empty() {
            return Ok(false);
        }

        // Validate event belongs to the specified room
        if event.room_id != room_id {
            return Ok(false);
        }

        // Basic event content validation
        if event.event_id.is_empty() || event.sender.is_empty() {
            return Ok(false);
        }

        // Additional validation could include:
        // - Event signature verification
        // - Auth event validation  
        // - State resolution
        Ok(true)
    }

    /// Check if a transaction ID has been used before
    pub async fn check_transaction_duplicate(&self, txn_id: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT count() FROM transaction WHERE txn_id = $txn_id GROUP ALL";
        let mut result = self.db.query(query).bind(("txn_id", txn_id.to_string())).await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    // Federation validation methods

    /// Validate event for federation
    pub async fn validate_event_for_federation(
        &self,
        event: &Event,
        room_version: &str,
    ) -> Result<EventValidationResult, RepositoryError> {
        // Basic event structure validation
        if event.event_id.is_empty() || event.room_id.is_empty() || event.sender.is_empty() {
            return Ok(EventValidationResult {
                valid: false,
                reason: Some("Missing required event fields".to_string()),
                error_code: Some("M_BAD_JSON".to_string()),
            });
        }

        // Validate event type
        if event.event_type.is_empty() {
            return Ok(EventValidationResult {
                valid: false,
                reason: Some("Missing event type".to_string()),
                error_code: Some("M_BAD_JSON".to_string()),
            });
        }

        // Validate room version compatibility
        match room_version {
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" => {
                // Supported room versions
            },
            _ => {
                return Ok(EventValidationResult {
                    valid: false,
                    reason: Some(format!("Unsupported room version: {}", room_version)),
                    error_code: Some("M_UNSUPPORTED_ROOM_VERSION".to_string()),
                });
            },
        }

        // Validate event ID format
        if !event.event_id.starts_with('$') {
            return Ok(EventValidationResult {
                valid: false,
                reason: Some("Invalid event ID format".to_string()),
                error_code: Some("M_INVALID_PARAM".to_string()),
            });
        }

        // Validate room ID format
        if !event.room_id.starts_with('!') {
            return Ok(EventValidationResult {
                valid: false,
                reason: Some("Invalid room ID format".to_string()),
                error_code: Some("M_INVALID_PARAM".to_string()),
            });
        }

        // Validate sender format
        if !event.sender.starts_with('@') {
            return Ok(EventValidationResult {
                valid: false,
                reason: Some("Invalid sender format".to_string()),
                error_code: Some("M_INVALID_PARAM".to_string()),
            });
        }

        // Check if event already exists (prevent duplicates)
        let existing_query = "SELECT event_id FROM event WHERE event_id = $event_id LIMIT 1";
        let mut result = self
            .db
            .query(existing_query)
            .bind(("event_id", event.event_id.clone()))
            .await?;
        let existing: Vec<serde_json::Value> = result.take(0)?;

        if !existing.is_empty() {
            return Ok(EventValidationResult {
                valid: false,
                reason: Some("Event already exists".to_string()),
                error_code: Some("M_DUPLICATE".to_string()),
            });
        }

        Ok(EventValidationResult { valid: true, reason: None, error_code: None })
    }

    /// Get auth events for an event
    pub async fn get_auth_events_for_event(
        &self,
        event: &Event,
    ) -> Result<Vec<Event>, RepositoryError> {
        if let Some(auth_event_ids) = &event.auth_events {
            let query = "SELECT * FROM event WHERE event_id IN $auth_event_ids";
            let mut result = self
                .db
                .query(query)
                .bind(("auth_event_ids", auth_event_ids.clone()))
                .await?;
            let auth_events: Vec<Event> = result.take(0)?;
            Ok(auth_events)
        } else {
            Ok(Vec::new())
        }
    }

    /// Validate event auth chain
    pub async fn validate_event_auth_chain(
        &self,
        event: &Event,
        auth_events: &[Event],
    ) -> Result<bool, RepositoryError> {
        // Basic auth chain validation

        // For membership events, check if there's a power levels event in auth chain
        if event.event_type == "m.room.member" {
            let has_power_levels =
                auth_events.iter().any(|e| e.event_type == "m.room.power_levels");

            if !has_power_levels {
                // Check if there's a create event (room creator can always invite)
                let has_create = auth_events.iter().any(|e| e.event_type == "m.room.create");

                if !has_create {
                    return Ok(false);
                }
            }
        }

        // For state events, check if sender has sufficient power level
        if event.state_key.is_some() {
            let power_repo = crate::repository::power_levels::PowerLevelsRepository::new(self.db.clone());
            let power_levels = power_repo.get_power_levels(&event.room_id).await?;

            let sender_level = power_levels
                .users
                .get(&event.sender)
                .copied()
                .unwrap_or(power_levels.users_default);

            let required_level = power_levels
                .events
                .get(&event.event_type)
                .copied()
                .unwrap_or(power_levels.state_default);

            if sender_level < required_level {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Validate event meets all Matrix requirements
    pub async fn validate_event(
        &self,
        event: &Event,
        room_id: &str,
    ) -> Result<bool, RepositoryError> {
        // 1. Validate required fields are present
        if event.event_id.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "event_id".to_string(),
                message: "Event ID cannot be empty".to_string(),
            });
        }

        if event.room_id.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "room_id".to_string(),
                message: "Room ID cannot be empty".to_string(),
            });
        }

        if event.sender.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "sender".to_string(),
                message: "Sender cannot be empty".to_string(),
            });
        }

        if event.event_type.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "type".to_string(),
                message: "Event type cannot be empty".to_string(),
            });
        }

        // 2. Validate sender is valid Matrix User ID format (@user:domain)
        if !event.sender.starts_with('@') || !event.sender.contains(':') {
            return Err(RepositoryError::ValidationError {
                field: "sender".to_string(),
                message: format!("Invalid Matrix user ID format: {}", event.sender),
            });
        }

        // 3. Validate state events have state_key
        if event.event_type.starts_with("m.room.")
            && event.event_type != "m.room.message"
            && event.event_type != "m.room.redaction"
            && event.state_key.is_none()
        {
            return Err(RepositoryError::ValidationError {
                field: "state_key".to_string(),
                message: format!("State event {} requires state_key", event.event_type),
            });
        }

        // 4. Validate event content matches type requirements
        match event.event_type.as_str() {
            "m.room.member" => {
                if event.state_key.is_none() {
                    return Err(RepositoryError::ValidationError {
                        field: "state_key".to_string(),
                        message: "m.room.member requires state_key".to_string(),
                    });
                }
                // Validate membership content has required 'membership' field
                if let EventContent::Unknown(ref content) = event.content {
                    if content.get("membership").is_none() {
                        return Err(RepositoryError::ValidationError {
                            field: "content.membership".to_string(),
                            message: "m.room.member requires membership field".to_string(),
                        });
                    }
                }
            }
            "m.room.power_levels" => {
                if let EventContent::Unknown(ref content) = event.content {
                    // Validate required power level fields
                    if content.get("users_default").is_none() {
                        return Err(RepositoryError::ValidationError {
                            field: "content.users_default".to_string(),
                            message: "Power levels require users_default".to_string(),
                        });
                    }
                }
            }
            "m.room.create" => {
                if event.state_key.as_deref() != Some("") {
                    return Err(RepositoryError::ValidationError {
                        field: "state_key".to_string(),
                        message: "m.room.create requires empty state_key".to_string(),
                    });
                }
            }
            _ => {}
        }

        // 5. Validate event belongs to specified room
        if event.room_id != room_id {
            return Err(RepositoryError::ValidationError {
                field: "room_id".to_string(),
                message: format!(
                    "Event room_id {} doesn't match {}",
                    event.room_id, room_id
                ),
            });
        }

        // 6. For state events, validate power levels
        if event.state_key.is_some() {
            let power_repo = crate::repository::power_levels::PowerLevelsRepository::new(self.db.clone());
            let power_levels = power_repo.get_power_levels(room_id).await?;

            let sender_level = power_levels
                .users
                .get(&event.sender)
                .copied()
                .unwrap_or(power_levels.users_default);

            let required_level = power_levels
                .events
                .get(&event.event_type)
                .copied()
                .unwrap_or(power_levels.state_default);

            if sender_level < required_level {
                return Err(RepositoryError::Forbidden {
                    reason: format!(
                        "User {} has power level {} but needs {} for {}",
                        event.sender, sender_level, required_level, event.event_type
                    ),
                });
            }
        }

        // 7. Validate signatures if present
        if let Some(ref signatures) = event.signatures {
            if signatures.is_empty() {
                tracing::warn!("Event has empty signatures field");
            }
            // Signature validation would go here
        }

        Ok(true)
    }

    /// Sign event for federation
    pub async fn sign_event(
        &self,
        event: &mut Event,
        server_name: &str,
        key_id: &str,
    ) -> Result<(), RepositoryError> {
        // Get server signing key
        let key_query = "
            SELECT private_key FROM server_signing_keys
            WHERE server_name = $server_name AND key_id = $key_id AND is_active = true
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(key_query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .await?;
        let keys: Vec<serde_json::Value> = result.take(0)?;

        if keys.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Server signing key".to_string(),
                id: format!("{}:{}", server_name, key_id),
            });
        }

        // Extract private key
        let private_key = keys[0]["private_key"]
            .as_str()
            .ok_or_else(|| RepositoryError::Validation {
                field: "private_key".to_string(),
                message: "Private key not found in result".to_string(),
            })?;

        // Convert event to JSON value for canonicalization
        let event_value = serde_json::to_value(&*event)
            .map_err(RepositoryError::Serialization)?;

        // Get canonical JSON (removes signatures and unsigned)
        let canonical = canonical_json_for_signing(&event_value)
            .map_err(|e| RepositoryError::SerializationError {
                message: format!("Canonical JSON error: {}", e),
            })?;

        // Decode signing key from base64
        let key_bytes = general_purpose::STANDARD.decode(private_key)
            .map_err(|e| RepositoryError::Validation {
                field: "signing_key".to_string(),
                message: format!("Invalid base64 signing key: {}", e),
            })?;

        let key_array: [u8; 32] = key_bytes.try_into()
            .map_err(|_| RepositoryError::Validation {
                field: "signing_key".to_string(),
                message: "Signing key must be 32 bytes".to_string(),
            })?;

        // Sign the canonical JSON
        let signing_key_obj = SigningKey::from_bytes(&key_array);
        let signature = signing_key_obj.sign(canonical.as_bytes());
        let signature_b64 = general_purpose::STANDARD.encode(signature.to_bytes());

        // Add signature to event
        let mut signatures = event.signatures.clone().unwrap_or_default();
        let server_sigs = signatures.entry(server_name.to_string()).or_default();
        server_sigs.insert(key_id.to_string(), signature_b64);
        event.signatures = Some(signatures);

        Ok(())
    }

    /// Verify event signatures
    pub async fn verify_event_signatures(
        &self,
        event: &Event,
    ) -> Result<SignatureValidation, RepositoryError> {
        let mut verified_signatures = HashMap::new();
        let mut missing_signatures = Vec::new();

        if let Some(signatures) = &event.signatures {
            for (server_name, server_sigs) in signatures {
                let mut server_verification = HashMap::new();

                for (key_id, signature) in server_sigs {
                    // Get server public key
                    let key_query = "
                        SELECT verify_key FROM server_keys
                        WHERE server_name = $server_name AND key_id = $key_id
                        AND (valid_until_ts IS NULL OR valid_until_ts > $now)
                        LIMIT 1
                    ";
                    let mut result = self
                        .db
                        .query(key_query)
                        .bind(("server_name", server_name.clone()))
                        .bind(("key_id", key_id.clone()))
                        .bind(("now", Utc::now()))
                        .await?;
                    let keys: Vec<serde_json::Value> = result.take(0)?;

                    if keys.is_empty() {
                        server_verification.insert(key_id.clone(), false);
                        missing_signatures.push(format!("{}:{}", server_name, key_id));
                    } else {
                        // Extract public key
                        let public_key = match keys[0]["verify_key"].as_str() {
                            Some(key) => key,
                            None => {
                                server_verification.insert(key_id.clone(), false);
                                continue;
                            }
                        };

                        // Decode signature and public key from base64
                        let sig_bytes = match general_purpose::STANDARD.decode(signature) {
                            Ok(b) => b,
                            Err(_) => {
                                server_verification.insert(key_id.clone(), false);
                                continue;
                            }
                        };

                        let key_bytes = match general_purpose::STANDARD.decode(public_key) {
                            Ok(b) => b,
                            Err(_) => {
                                server_verification.insert(key_id.clone(), false);
                                continue;
                            }
                        };

                        // Verify signature
                        if sig_bytes.len() == 64 && key_bytes.len() == 32 {
                            let key_array: [u8; 32] = match key_bytes.try_into() {
                                Ok(arr) => arr,
                                Err(_) => {
                                    server_verification.insert(key_id.clone(), false);
                                    continue;
                                }
                            };
                            let sig_array: [u8; 64] = match sig_bytes.try_into() {
                                Ok(arr) => arr,
                                Err(_) => {
                                    server_verification.insert(key_id.clone(), false);
                                    continue;
                                }
                            };

                            match VerifyingKey::from_bytes(&key_array) {
                                Ok(verifying_key) => {
                                    let signature_obj = Signature::from_bytes(&sig_array);

                                    // Get canonical JSON
                                    let event_value = match serde_json::to_value(event) {
                                        Ok(v) => v,
                                        Err(_) => {
                                            server_verification.insert(key_id.clone(), false);
                                            continue;
                                        }
                                    };

                                    let canonical = match canonical_json_for_signing(&event_value) {
                                        Ok(c) => c,
                                        Err(_) => {
                                            server_verification.insert(key_id.clone(), false);
                                            continue;
                                        }
                                    };

                                    let valid = verifying_key.verify(canonical.as_bytes(), &signature_obj).is_ok();
                                    server_verification.insert(key_id.clone(), valid);
                                }
                                Err(_) => {
                                    server_verification.insert(key_id.clone(), false);
                                }
                            }
                        } else {
                            server_verification.insert(key_id.clone(), false);
                        }
                    }
                }

                verified_signatures.insert(server_name.clone(), server_verification);
            }
        }

        let all_valid = verified_signatures
            .values()
            .all(|server_sigs| server_sigs.values().all(|&valid| valid));

        Ok(SignatureValidation {
            valid: all_valid,
            verified_signatures,
            missing_signatures,
        })
    }

    /// Get event reference hash
    pub async fn get_event_reference_hash(&self, event: &Event) -> Result<String, RepositoryError> {
        // Convert event to JSON Value
        let mut event_value = serde_json::to_value(event)
            .map_err(RepositoryError::Serialization)?;

        // Remove fields per Matrix spec
        if let Some(obj) = event_value.as_object_mut() {
            obj.remove("hashes");
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        // Get canonical JSON
        let canonical = canonical_json_for_signing(&event_value)
            .map_err(|e| RepositoryError::SerializationError {
                message: format!("Canonical JSON error: {}", e),
            })?;

        // Calculate SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();

        // Encode as UNPADDED base64 per Matrix spec
        let hash_b64 = general_purpose::STANDARD_NO_PAD.encode(hash);

        Ok(hash_b64)
    }

    /// Validate event content for specific event types
    pub async fn validate_event_content(
        &self,
        event: &Event,
    ) -> Result<EventValidationResult, RepositoryError> {
        match event.event_type.as_str() {
            "m.room.member" => {
                // Validate membership event content
                if let EventContent::Unknown(content) = &event.content
                    && let Some(membership) = content.get("membership").and_then(|v| v.as_str()) {
                        match membership {
                            "join" | "leave" | "invite" | "ban" | "knock" => {
                                return Ok(EventValidationResult {
                                    valid: true,
                                    reason: None,
                                    error_code: None,
                                });
                            },
                            _ => {
                                return Ok(EventValidationResult {
                                    valid: false,
                                    reason: Some("Invalid membership value".to_string()),
                                    error_code: Some("M_BAD_JSON".to_string()),
                                });
                            },
                        }
                    }

                Ok(EventValidationResult {
                    valid: false,
                    reason: Some("Missing membership field".to_string()),
                    error_code: Some("M_BAD_JSON".to_string()),
                })
            },
            "m.room.create" => {
                // Validate room creation event
                if let EventContent::Unknown(content) = &event.content
                    && content.get("creator").and_then(|v| v.as_str()).is_some() {
                        return Ok(EventValidationResult {
                            valid: true,
                            reason: None,
                            error_code: None,
                        });
                    }

                Ok(EventValidationResult {
                    valid: false,
                    reason: Some("Missing creator field".to_string()),
                    error_code: Some("M_BAD_JSON".to_string()),
                })
            },
            _ => {
                // For other event types, basic validation
                Ok(EventValidationResult { valid: true, reason: None, error_code: None })
            },
        }
    }

    /// Get events by reference hash (for federation deduplication)
    pub async fn get_events_by_hash(&self, hash: &str) -> Result<Vec<Event>, RepositoryError> {
        let query = "SELECT * FROM event WHERE content_hash = $hash";
        let mut result = self.db.query(query).bind(("hash", hash.to_string())).await?;
        let events: Vec<Event> = result.take(0)?;
        Ok(events)
    }

    /// Store event with hash for federation
    pub async fn store_event_with_hash(&self, event: &Event) -> Result<(), RepositoryError> {
        let hash = self.get_event_reference_hash(event).await?;

        let query = "
            CREATE event SET
            event_id = $event_id,
            room_id = $room_id,
            sender = $sender,
            event_type = $event_type,
            content = $content,
            state_key = $state_key,
            origin_server_ts = $origin_server_ts,
            content_hash = $content_hash,
            signatures = $signatures,
            auth_events = $auth_events,
            prev_events = $prev_events,
            depth = $depth
        ";

        self.db
            .query(query)
            .bind(("event_id", event.event_id.clone()))
            .bind(("room_id", event.room_id.clone()))
            .bind(("sender", event.sender.clone()))
            .bind(("event_type", event.event_type.clone()))
            .bind(("content", serde_json::to_value(&event.content)?))
            .bind(("state_key", event.state_key.clone()))
            .bind(("origin_server_ts", event.origin_server_ts))
            .bind(("content_hash", hash))
            .bind(("signatures", event.signatures.clone()))
            .bind(("auth_events", event.auth_events.clone()))
            .bind(("prev_events", event.prev_events.clone()))
            .bind(("depth", event.depth))
            .await?;

        Ok(())
    }

    // EXTENDED EVENT CONTEXT AND RELATIONS METHODS - SUBTASK 3

    /// Get event context around a specific event
    pub async fn get_event_context(
        &self,
        room_id: &str,
        event_id: &str,
        limit: u32,
    ) -> Result<EventContext, RepositoryError> {
        // Get the target event
        let target_event_query =
            "SELECT * FROM event WHERE room_id = $room_id AND event_id = $event_id LIMIT 1";
        let mut result = self
            .db
            .query(target_event_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let target_events: Vec<Event> = result.take(0)?;
        let target_event = target_events.into_iter().next();

        let target_timestamp = if let Some(ref event) = target_event {
            event.origin_server_ts
        } else {
            return Err(RepositoryError::NotFound {
                entity_type: "Event".to_string(),
                id: event_id.to_string(),
            });
        };

        // Get events before
        let before_query = format!(
            "SELECT * FROM event WHERE room_id = $room_id AND origin_server_ts < $timestamp ORDER BY origin_server_ts DESC LIMIT {}",
            limit
        );
        let mut before_result = self
            .db
            .query(&before_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("timestamp", target_timestamp))
            .await?;
        let mut events_before: Vec<Event> = before_result.take(0)?;
        events_before.reverse(); // Reverse to get chronological order

        // Get events after
        let after_query = format!(
            "SELECT * FROM event WHERE room_id = $room_id AND origin_server_ts > $timestamp ORDER BY origin_server_ts ASC LIMIT {}",
            limit
        );
        let mut after_result = self
            .db
            .query(&after_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("timestamp", target_timestamp))
            .await?;
        let events_after: Vec<Event> = after_result.take(0)?;

        // Get room state at time of event
        let state_query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND state_key IS NOT NULL
            AND origin_server_ts <= $timestamp
            ORDER BY event_type, state_key, origin_server_ts DESC
        ";
        let mut state_result = self
            .db
            .query(state_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("timestamp", target_timestamp))
            .await?;
        let all_state_events: Vec<Event> = state_result.take(0)?;

        // Deduplicate state events by (event_type, state_key) keeping the latest
        let mut state_map = HashMap::new();
        for event in all_state_events {
            let key = (event.event_type.clone(), event.state_key.clone().unwrap_or_default());
            state_map.entry(key).or_insert(event);
        }
        let state: Vec<Event> = state_map.into_values().collect();

        // Generate tokens from full event context
        let mut all_events = events_before.clone();
        if let Some(ref evt) = target_event {
            all_events.push(evt.clone());
        }
        all_events.extend(events_after.clone());

        let (start, end) = crate::pagination::generate_timeline_tokens(&all_events, room_id);

        Ok(EventContext {
            events_before,
            event: target_event,
            events_after,
            start,
            end,
            state,
        })
    }

    /// Get events before a specific event
    pub async fn get_events_before(
        &self,
        room_id: &str,
        event_id: &str,
        limit: u32,
    ) -> Result<Vec<Event>, RepositoryError> {
        // First get the timestamp of the target event
        let target_event_query = "SELECT origin_server_ts FROM event WHERE room_id = $room_id AND event_id = $event_id LIMIT 1";
        let mut result = self
            .db
            .query(target_event_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let timestamps: Vec<serde_json::Value> = result.take(0)?;

        let target_timestamp = timestamps
            .first()
            .and_then(|v| v.get("origin_server_ts"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Event".to_string(),
                    id: event_id.to_string(),
                }
            })?;

        // Get events before the target timestamp
        let before_query = format!(
            "SELECT * FROM event WHERE room_id = $room_id AND origin_server_ts < $timestamp ORDER BY origin_server_ts DESC LIMIT {}",
            limit
        );
        let mut before_result = self
            .db
            .query(&before_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("timestamp", target_timestamp))
            .await?;
        let mut events_before: Vec<Event> = before_result.take(0)?;
        events_before.reverse(); // Reverse to get chronological order

        Ok(events_before)
    }

    /// Get events after a specific event
    pub async fn get_events_after(
        &self,
        room_id: &str,
        event_id: &str,
        limit: u32,
    ) -> Result<Vec<Event>, RepositoryError> {
        // First get the timestamp of the target event
        let target_event_query = "SELECT origin_server_ts FROM event WHERE room_id = $room_id AND event_id = $event_id LIMIT 1";
        let mut result = self
            .db
            .query(target_event_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let timestamps: Vec<serde_json::Value> = result.take(0)?;

        let target_timestamp = timestamps
            .first()
            .and_then(|v| v.get("origin_server_ts"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Event".to_string(),
                    id: event_id.to_string(),
                }
            })?;

        // Get events after the target timestamp
        let after_query = format!(
            "SELECT * FROM event WHERE room_id = $room_id AND origin_server_ts > $timestamp ORDER BY origin_server_ts ASC LIMIT {}",
            limit
        );
        let mut after_result = self
            .db
            .query(&after_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("timestamp", target_timestamp))
            .await?;
        let events_after: Vec<Event> = after_result.take(0)?;

        Ok(events_after)
    }

    /// Report an event for moderation
    pub async fn report_event(
        &self,
        room_id: &str,
        event_id: &str,
        reporter_id: &str,
        reason: &str,
        score: Option<i32>,
    ) -> Result<(), RepositoryError> {
        // Verify the event exists
        let event_exists_query =
            "SELECT event_id FROM event WHERE room_id = $room_id AND event_id = $event_id LIMIT 1";
        let mut result = self
            .db
            .query(event_exists_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if events.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Event".to_string(),
                id: event_id.to_string(),
            });
        }

        // Check if user has already reported this event
        let existing_report_query = "SELECT id FROM event_reports WHERE event_id = $event_id AND reporter_id = $reporter_id LIMIT 1";
        let mut existing_result = self
            .db
            .query(existing_report_query)
            .bind(("event_id", event_id.to_string()))
            .bind(("reporter_id", reporter_id.to_string()))
            .await?;
        let existing_reports: Vec<serde_json::Value> = existing_result.take(0)?;

        if !existing_reports.is_empty() {
            return Err(RepositoryError::Conflict {
                message: format!("Event already reported by user {}", reporter_id),
            });
        }

        // Create the report
        let report_id = format!("report_{}", Uuid::new_v4());
        let insert_query = "
            INSERT INTO event_reports (
                id, event_id, room_id, reporter_id, reason, score, created_at
            ) VALUES (
                $id, $event_id, $room_id, $reporter_id, $reason, $score, $created_at
            )
        ";

        self.db
            .query(insert_query)
            .bind(("id", report_id))
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("reporter_id", reporter_id.to_string()))
            .bind(("reason", reason.to_string()))
            .bind(("score", score))
            .bind(("created_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Get reports for an event
    pub async fn get_event_reports(
        &self,
        event_id: &str,
    ) -> Result<Vec<EventReport>, RepositoryError> {
        let query =
            "SELECT * FROM event_reports WHERE event_id = $event_id ORDER BY created_at DESC";
        let mut result = self.db.query(query).bind(("event_id", event_id.to_string())).await?;
        let reports_data: Vec<serde_json::Value> = result.take(0)?;

        let mut reports = Vec::new();
        for report_data in reports_data {
            let report = EventReport {
                event_id: report_data
                    .get("event_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                reporter_id: report_data
                    .get("reporter_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                reason: report_data
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                score: report_data.get("score").and_then(|v| v.as_i64()).map(|v| v as i32),
                created_at: report_data
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(Utc::now),
            };
            reports.push(report);
        }

        Ok(reports)
    }

    /// Get related events (replies, reactions, etc.)
    pub async fn get_related_events(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        // Get events that relate to the target event
        let query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND content.relates_to.event_id = $event_id
            ORDER BY origin_server_ts ASC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let related_events: Vec<Event> = result.take(0)?;

        Ok(related_events)
    }

    /// Get thread events for a specific event
    pub async fn get_thread_events(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        // Get events that are part of the thread rooted at this event
        let query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND (
                content.relates_to.event_id = $event_id
                OR content.relates_to.thread_root = $event_id
            )
            ORDER BY origin_server_ts ASC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let thread_events: Vec<Event> = result.take(0)?;

        Ok(thread_events)
    }

    /// Get reactions to an event
    pub async fn get_event_reactions(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        // Get reaction events for the target event
        let query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.reaction'
            AND content.relates_to.event_id = $event_id
            ORDER BY origin_server_ts ASC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let reactions: Vec<Event> = result.take(0)?;

        Ok(reactions)
    }

    /// Get edits/replacements for an event
    pub async fn get_event_edits(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        // Get edit events for the target event
        let query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND content.relates_to.rel_type = 'm.replace'
            AND content.relates_to.event_id = $event_id
            ORDER BY origin_server_ts ASC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let edits: Vec<Event> = result.take(0)?;

        Ok(edits)
    }

    /// Get the latest version of an event (considering edits)
    pub async fn get_event_with_edits(
        &self,
        event_id: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        // Get the original event
        let mut event = match self.get_by_id(event_id).await? {
            Some(e) => e,
            None => return Ok(None),
        };

        // Get the latest edit
        let edits = self.get_event_edits(&event.room_id, event_id).await?;
        if let Some(latest_edit) = edits.last() {
            // Apply the edit to the original event content
            if let EventContent::Unknown(edit_content) = &latest_edit.content
                && let Some(new_content) = edit_content.get("m.new_content") {
                    event.content = EventContent::Unknown(new_content.clone());
                }
        }

        Ok(Some(event))
    }

    /// Check if a room is world-readable based on history visibility
    pub async fn is_room_world_readable(&self, room_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT content.history_visibility
            FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.history_visibility'
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct HistoryVisibility {
            history_visibility: Option<String>,
        }

        let visibility: Option<HistoryVisibility> = response.take(0)?;
        let history_visibility = visibility
            .and_then(|v| v.history_visibility)
            .unwrap_or_else(|| "shared".to_string());

        Ok(history_visibility == "world_readable")
    }

    /// Get room state at a specific event depth
    pub async fn get_room_state_at_event(&self, room_id: &str, event_id: &str) -> Result<Vec<serde_json::Value>, RepositoryError> {
        // Get the target event's depth for state resolution
        let depth_query = "
            SELECT depth
            FROM event
            WHERE event_id = $event_id
        ";

        let mut response = self.db.query(depth_query)
            .bind(("event_id", event_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct EventDepth {
            depth: Option<i64>,
        }

        let event_depth: Option<EventDepth> = response.take(0)?;
        let target_depth = event_depth
            .and_then(|e| e.depth)
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Event depth".to_string(),
                id: event_id.to_string(),
            })?;

        // Get state events at or before the target event depth
        let state_query = "
            SELECT *
            FROM event
            WHERE room_id = $room_id
            AND state_key IS NOT NULL
            AND depth <= $target_depth
            AND (
                SELECT COUNT()
                FROM event e2
                WHERE e2.room_id = $room_id
                AND e2.event_type = event.event_type
                AND e2.state_key = event.state_key
                AND e2.depth <= $target_depth
                AND (e2.depth > event.depth OR (e2.depth = event.depth AND e2.origin_server_ts > event.origin_server_ts))
            ) = 0
            ORDER BY event_type, state_key
        ";

        let mut response = self.db.query(state_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("target_depth", target_depth))
            .await?;

        let events: Vec<Event> = response.take(0)?;

        // Convert events to JSON format for response
        let state_events: Vec<serde_json::Value> = events
            .into_iter()
            .map(|event| serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({})))
            .collect();

        Ok(state_events)
    }

    /// Get auth chain for a set of state events
    pub async fn get_auth_chain_for_state(&self, state_events: &[serde_json::Value]) -> Result<Vec<serde_json::Value>, RepositoryError> {
        use std::collections::HashSet;

        let mut auth_event_ids = HashSet::new();
        let mut to_process = HashSet::new();

        // Collect initial auth events
        for state_event in state_events {
            if let Some(auth_events) = state_event.get("auth_events").and_then(|v| v.as_array()) {
                for auth_event in auth_events {
                    if let Some(auth_event_id) = auth_event.as_str()
                        && auth_event_ids.insert(auth_event_id.to_string()) {
                            to_process.insert(auth_event_id.to_string());
                        }
                }
            }
        }

        // Recursively fetch auth events
        while !to_process.is_empty() {
            let current_batch: Vec<String> = to_process.drain().collect();

            let query = "
                SELECT *
                FROM event
                WHERE event_id IN $auth_event_ids
            ";

            let mut response = self.db.query(query)
                .bind(("auth_event_ids", current_batch))
                .await?;

            let events: Vec<Event> = response.take(0)?;

            // Process auth events of the fetched events
            for event in &events {
                if let Some(auth_events) = &event.auth_events {
                    for auth_event_id in auth_events {
                        if auth_event_ids.insert(auth_event_id.clone()) {
                            to_process.insert(auth_event_id.clone());
                        }
                    }
                }
            }
        }

        if auth_event_ids.is_empty() {
            return Ok(vec![]);
        }

        // Fetch all auth events
        let auth_ids: Vec<String> = auth_event_ids.into_iter().collect();

        let query = "
            SELECT *
            FROM event
            WHERE event_id IN $auth_event_ids
            ORDER BY depth, origin_server_ts
        ";

        let mut response = self.db.query(query)
            .bind(("auth_event_ids", auth_ids))
            .await?;

        let events: Vec<Event> = response.take(0)?;

        // Convert events to JSON format for response
        let auth_chain: Vec<serde_json::Value> = events
            .into_iter()
            .map(|event| serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({})))
            .collect();

        Ok(auth_chain)
    }

    /// Mark an event as read by a user
    pub async fn mark_event_as_read(
        &self,
        room_id: &str,
        event_id: &str,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE room_account_data
            SET content.read_marker = $event_id, updated_at = $updated_at
            WHERE room_id = $room_id AND user_id = $user_id AND data_type = 'm.fully_read'
        ";

        self.db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Get unread events for a user in a room
    pub async fn get_unread_events(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        // Get the user's read marker
        let read_marker_query = "
            SELECT content.read_marker FROM room_account_data
            WHERE room_id = $room_id AND user_id = $user_id AND data_type = 'm.fully_read'
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(read_marker_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let read_markers: Vec<serde_json::Value> = result.take(0)?;

        let read_event_id = read_markers
            .first()
            .and_then(|v| v.get("read_marker"))
            .and_then(|v| v.as_str());

        if let Some(read_event_id) = read_event_id {
            // Get the timestamp of the read event
            let read_timestamp_query =
                "SELECT origin_server_ts FROM event WHERE event_id = $event_id LIMIT 1";
            let mut ts_result = self
                .db
                .query(read_timestamp_query)
                .bind(("event_id", read_event_id.to_string()))
                .await?;
            let timestamps: Vec<serde_json::Value> = ts_result.take(0)?;

            if let Some(read_timestamp) = timestamps
                .first()
                .and_then(|v| v.get("origin_server_ts"))
                .and_then(|v| v.as_i64())
            {
                // Get events after the read timestamp
                let unread_query = "
                    SELECT * FROM event
                    WHERE room_id = $room_id
                    AND origin_server_ts > $read_timestamp
                    AND sender != $user_id
                    ORDER BY origin_server_ts ASC
                ";
                let mut unread_result = self
                    .db
                    .query(unread_query)
                    .bind(("room_id", room_id.to_string()))
                    .bind(("read_timestamp", read_timestamp))
                    .bind(("user_id", user_id.to_string()))
                    .await?;
                let unread_events: Vec<Event> = unread_result.take(0)?;
                return Ok(unread_events);
            }
        }

        // If no read marker, return all events except user's own
        let all_events_query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND sender != $user_id
            ORDER BY origin_server_ts ASC
        ";
        let mut all_result = self
            .db
            .query(all_events_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let all_events: Vec<Event> = all_result.take(0)?;
        Ok(all_events)
    }

    // EVENT REPLACEMENT AND REDACTION METHODS - SUBTASK 8

    /// Create a replacement event for editing a previous event
    pub async fn create_replacement_event(
        &self,
        room_id: &str,
        original_event_id: &str,
        replacement_event: &Event,
    ) -> Result<(), RepositoryError> {
        // Verify the original event exists
        let original_event = self.get_by_id(original_event_id).await?;
        if original_event.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Original event".to_string(),
                id: original_event_id.to_string(),
            });
        }

        // Verify replacement event has proper relates_to structure
        if let EventContent::Unknown(content) = &replacement_event.content {
            if let Some(relates_to) = content.get("m.relates_to") {
                if relates_to.get("rel_type").and_then(|v| v.as_str()) != Some("m.replace") {
                    return Err(RepositoryError::Validation {
                        field: "m.relates_to.rel_type".to_string(),
                        message: "Replacement event must have rel_type 'm.replace'".to_string(),
                    });
                }

                if relates_to.get("event_id").and_then(|v| v.as_str()) != Some(original_event_id) {
                    return Err(RepositoryError::Validation {
                        field: "m.relates_to.event_id".to_string(),
                        message: "Replacement event must reference the original event".to_string(),
                    });
                }
            } else {
                return Err(RepositoryError::Validation {
                    field: "m.relates_to".to_string(),
                    message: "Replacement event must have relates_to field".to_string(),
                });
            }
        }

        // Store the replacement event
        self.create(replacement_event).await?;

        // Create event relation
        let relation_query = "
            CREATE event_relations SET
                event_id = $replacement_event_id,
                relates_to_event_id = $original_event_id,
                rel_type = 'm.replace',
                room_id = $room_id,
                sender = $sender,
                created_at = time::now()
        ";

        self.db
            .query(relation_query)
            .bind(("replacement_event_id", replacement_event.event_id.clone()))
            .bind(("original_event_id", original_event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", replacement_event.sender.clone()))
            .await?;

        Ok(())
    }

    /// Get all replacement events for a given event
    pub async fn get_event_replacements(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT e.* FROM event e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $event_id
            AND r.rel_type = 'm.replace'
            AND e.room_id = $room_id
            ORDER BY e.origin_server_ts ASC
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let replacements: Vec<Event> = result.take(0)?;
        Ok(replacements)
    }

    /// Get the latest version of an event considering replacements
    pub async fn get_latest_event_version(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Event, RepositoryError> {
        // Get the original event
        let mut event = self.get_by_id(event_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Event".to_string(),
                id: event_id.to_string(),
            }
        })?;

        // Get the latest replacement
        let replacements = self.get_event_replacements(room_id, event_id).await?;

        if let Some(latest_replacement) = replacements.last() {
            // Apply the replacement content
            if let EventContent::Unknown(replacement_content) = &latest_replacement.content
                && let Some(new_content) = replacement_content.get("m.new_content") {
                    event.content = EventContent::Unknown(new_content.clone());
                }
        }

        Ok(event)
    }

    /// Redact an event by creating a redaction event
    pub async fn redact_event(
        &self,
        room_id: &str,
        event_id: &str,
        redaction_event: &Event,
    ) -> Result<(), RepositoryError> {
        // Verify the target event exists
        let target_event = self.get_by_id(event_id).await?;
        if target_event.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Target event".to_string(),
                id: event_id.to_string(),
            });
        }

        // Verify redaction event type
        if redaction_event.event_type != "m.room.redaction" {
            return Err(RepositoryError::Validation {
                field: "event_type".to_string(),
                message: "Redaction event must have type 'm.room.redaction'".to_string(),
            });
        }

        // Verify redaction event has proper redacts field
        if redaction_event.redacts.as_ref() != Some(&event_id.to_string()) {
            return Err(RepositoryError::Validation {
                field: "redacts".to_string(),
                message: "Redaction event must reference the target event in redacts field"
                    .to_string(),
            });
        }

        // Store the redaction event
        self.create(redaction_event).await?;

        // Mark the original event as redacted
        let redact_query = "
            UPDATE event SET
                redacted = true,
                redacted_by = $redaction_event_id,
                redacted_at = time::now()
            WHERE event_id = $event_id AND room_id = $room_id
        ";

        self.db
            .query(redact_query)
            .bind(("redaction_event_id", redaction_event.event_id.clone()))
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        Ok(())
    }

    /// Check if an event has been redacted
    pub async fn is_event_redacted(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT redacted FROM event
            WHERE event_id = $event_id AND room_id = $room_id
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event_data) = events.first() {
            Ok(event_data.get("redacted").and_then(|v| v.as_bool()).unwrap_or(false))
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "Event".to_string(),
                id: event_id.to_string(),
            })
        }
    }

    /// Get the redaction event for a redacted event
    pub async fn get_redaction_event(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT redaction.* FROM event AS target
            JOIN event AS redaction ON target.redacted_by = redaction.event_id
            WHERE target.event_id = $event_id AND target.room_id = $room_id
            AND target.redacted = true
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let redaction_events: Vec<Event> = result.take(0)?;
        Ok(redaction_events.into_iter().next())
    }

    /// Get previous events for DAG construction (forward extremities)
    pub async fn get_prev_events(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = r#"
            SELECT VALUE event_id FROM event 
            WHERE room_id = $room_id 
            AND event_id NOT IN (
                SELECT VALUE unnest(prev_events) FROM event WHERE room_id = $room_id
            )
            ORDER BY origin_server_ts DESC 
            LIMIT 20
        "#;

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let events: Vec<String> = result.take(0)?;
        Ok(events)
    }

    /// Get auth events for event authorization
    pub async fn get_auth_events(
        &self,
        room_id: &str,
        event_type: &str,
        sender: &str,
        state_key: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let mut auth_events = Vec::new();

        // Always include the room creation event
        let create_query = "SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.create' LIMIT 1";
        let mut result = self.db.query(create_query).bind(("room_id", room_id.to_string())).await?;
        let create_events: Vec<String> = result.take(0)?;
        auth_events.extend(create_events);

        // Include current power levels
        let power_query = "SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.power_levels' AND state_key = '' ORDER BY origin_server_ts DESC LIMIT 1";
        let mut result = self.db.query(power_query).bind(("room_id", room_id.to_string())).await?;
        let power_events: Vec<String> = result.take(0)?;
        auth_events.extend(power_events);

        // Include sender's membership event
        let member_query = "SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.member' AND state_key = $sender ORDER BY origin_server_ts DESC LIMIT 1";
        let mut result = self
            .db
            .query(member_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .await?;
        let member_events: Vec<String> = result.take(0)?;
        auth_events.extend(member_events);

        // For membership events, include join rules
        if event_type == "m.room.member" {
            let join_query = "SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.join_rules' AND state_key = '' ORDER BY origin_server_ts DESC LIMIT 1";
            let mut result =
                self.db.query(join_query).bind(("room_id", room_id.to_string())).await?;
            let join_events: Vec<String> = result.take(0)?;
            auth_events.extend(join_events);

            // For m.room.member events, include target user's current membership if different from sender
            if state_key != sender {
                let target_query = "SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.member' AND state_key = $target ORDER BY origin_server_ts DESC LIMIT 1";
                let mut result = self
                    .db
                    .query(target_query)
                    .bind(("room_id", room_id.to_string()))
                    .bind(("target", state_key.to_string()))
                    .await?;
                let target_events: Vec<String> = result.take(0)?;
                auth_events.extend(target_events);
            }
        }

        Ok(auth_events)
    }

    /// Calculate event depth based on previous events
    pub async fn calculate_event_depth(
        &self,
        prev_events: &[String],
    ) -> Result<i64, RepositoryError> {
        if prev_events.is_empty() {
            return Ok(1);
        }

        let query = "SELECT VALUE depth FROM event WHERE event_id IN $prev_events ORDER BY depth DESC LIMIT 1";
        let mut result = self.db.query(query).bind(("prev_events", prev_events.to_vec())).await?;

        let max_depths: Vec<i64> = result.take(0)?;
        let max_depth = max_depths.into_iter().next().unwrap_or(0);
        Ok(max_depth + 1)
    }

    /// Check transaction idempotency
    pub async fn check_transaction_idempotency(
        &self,
        user_id: &str,
        txn_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT VALUE event_id FROM transaction_mapping WHERE user_id = $user_id AND txn_id = $txn_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("txn_id", txn_id.to_string()))
            .await?;

        let event_ids: Vec<String> = result.take(0)?;
        Ok(event_ids.into_iter().next())
    }

    /// Store transaction mapping for idempotency
    pub async fn store_transaction_mapping(
        &self,
        user_id: &str,
        txn_id: &str,
        event_id: &str,
    ) -> Result<(), RepositoryError> {
        let mapping = serde_json::json!({
            "user_id": user_id,
            "txn_id": txn_id,
            "event_id": event_id,
            "created_at": chrono::Utc::now()
        });

        let _: Option<Value> = self.db.create("transaction_mapping").content(mapping).await?;

        Ok(())
    }

    /// Create a complete event with DAG relationships, depth, and Matrix compliance
    pub async fn create_complete_event(
        &self,
        room_id: &str,
        event_type: &str,
        sender: &str,
        content: Value,
        state_key: Option<String>,
        txn_id: Option<String>,
    ) -> Result<Event, RepositoryError> {
        // Check transaction idempotency if txn_id provided
        if let Some(ref txn) = txn_id
            && let Some(existing_event_id) = self.check_transaction_idempotency(sender, txn).await?
                && let Some(existing_event) = self.get_by_id(&existing_event_id).await? {
                    return Ok(existing_event);
                }

        // Get previous events for DAG construction
        let prev_events = self.get_prev_events(room_id).await?;

        // Get auth events for authorization
        let state_key_str = state_key.as_deref().unwrap_or("");
        let auth_events = self.get_auth_events(room_id, event_type, sender, state_key_str).await?;

        // Calculate event depth
        let depth = self.calculate_event_depth(&prev_events).await?;

        // Generate event ID
        let event_id = format!("${}:example.com", Uuid::new_v4());

        // Create event with full DAG relationships
        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: sender.to_string(),
            event_type: event_type.to_string(),
            content: EventContent::Unknown(content),
            state_key,
            origin_server_ts: chrono::Utc::now().timestamp_millis(),
            unsigned: None,
            prev_events: if prev_events.is_empty() {
                None
            } else {
                Some(prev_events)
            },
            auth_events: if auth_events.is_empty() {
                None
            } else {
                Some(auth_events)
            },
            depth: Some(depth),
            hashes: None,     // To be filled by event signing
            signatures: None, // To be filled by event signing
            redacts: None,
            outlier: Some(false),
            rejected_reason: None,
            soft_failed: Some(false),
            received_ts: Some(chrono::Utc::now().timestamp_millis()),
        };

        // Create the event in database
        let created_event = self.create(&event).await?;

        // Store transaction mapping if provided
        if let Some(txn) = txn_id {
            self.store_transaction_mapping(sender, &txn, &event_id).await?;
        }

        Ok(created_event)
    }

    /// Get auth events specifically for join operations
    pub async fn get_auth_events_for_join(
        &self,
        room_id: &str,
        _user_id: &str, // May be used for future ACL checks
    ) -> Result<Vec<String>, RepositoryError> {
        let query = r#"
            SELECT event_id FROM event 
            WHERE room_id = $room_id 
            AND event_type IN ['m.room.create', 'm.room.join_rules', 'm.room.power_levels']
            AND state_key = ''
            ORDER BY origin_server_ts ASC
        "#;

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let auth_events: Vec<String> = result.take(0)?;
        Ok(auth_events)
    }



    /// Create a membership event with full Matrix protocol compliance for federation
    pub async fn create_federation_membership_event(
        &self,
        params: FederationMembershipParams<'_>,
    ) -> Result<String, RepositoryError> {
        use serde_json::json;
        use uuid::Uuid;

        let event_id = format!("${}:{}", Uuid::new_v4(), params.homeserver_name);

        let mut content = json!({
            "membership": match params.membership {
                matryx_entity::types::MembershipState::Join => "join",
                matryx_entity::types::MembershipState::Leave => "leave",
                matryx_entity::types::MembershipState::Invite => "invite",
                matryx_entity::types::MembershipState::Ban => "ban",
                matryx_entity::types::MembershipState::Knock => "knock",
            }
        });

        if let Some(reason) = params.reason {
            content["reason"] = json!(reason);
        }

        // Create the event
        let event = matryx_entity::types::Event {
            event_id: event_id.clone(),
            room_id: params.room_id.to_string(),
            sender: params.sender.to_string(),
            event_type: "m.room.member".to_string(),
            content: matryx_entity::types::EventContent::Unknown(content),
            state_key: Some(params.target.to_string()),
            origin_server_ts: chrono::Utc::now().timestamp_millis(),
            unsigned: None,
            prev_events: Some(params.prev_events.to_vec()),
            auth_events: Some(params.auth_events.to_vec()),
            depth: Some(params.depth),
            hashes: None,     // To be filled by signing process
            signatures: None, // To be filled by signing process
            redacts: None,
            outlier: Some(false),
            rejected_reason: None,
            soft_failed: Some(false),
            received_ts: Some(chrono::Utc::now().timestamp_millis()),
        };

        // Store the event in database
        let _: Option<matryx_entity::types::Event> = self
            .db
            .create(("event", &event_id))
            .content(event)
            .await
            .map_err(RepositoryError::Database)?;

        Ok(event_id)
    }

    /// Get auth events specifically for knock operations
    pub async fn get_auth_events_for_knock(
        &self,
        room_id: &str,
        _user_id: &str, // May be used for future ACL checks
    ) -> Result<Vec<String>, RepositoryError> {
        // Get the auth events needed for a knock event:
        // - m.room.create event
        // - m.room.join_rules event (to validate knocking is allowed)
        let query = r#"
            SELECT event_id FROM event 
            WHERE room_id = $room_id 
            AND event_type IN ['m.room.create', 'm.room.join_rules']
            AND state_key = ''
            ORDER BY origin_server_ts ASC
        "#;

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let auth_events: Vec<String> = result.take(0)?;
        Ok(auth_events)
    }

    /// Get room history visibility setting
    pub async fn get_room_history_visibility(
        &self,
        room_id: &str,
    ) -> Result<String, RepositoryError> {
        let query = "
            SELECT content.history_visibility
            FROM event
            WHERE room_id = $room_id
            AND type = 'm.room.history_visibility'
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        #[derive(serde::Deserialize)]
        struct HistoryVisibility {
            history_visibility: Option<String>,
        }

        let visibility: Option<HistoryVisibility> = response.take(0)?;
        Ok(visibility
            .and_then(|v| v.history_visibility)
            .unwrap_or_else(|| "shared".to_string()))
    }

    /// Get room join rules setting
    pub async fn get_room_join_rules(
        &self,
        room_id: &str,
    ) -> Result<String, RepositoryError> {
        let query = "
            SELECT content.join_rule
            FROM event
            WHERE room_id = $room_id
            AND type = 'm.room.join_rules'
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        #[derive(serde::Deserialize)]
        struct JoinRules {
            join_rule: Option<String>,
        }

        let join_rules: Option<JoinRules> = response.take(0)?;
        Ok(join_rules
            .and_then(|j| j.join_rule)
            .unwrap_or_else(|| "invite".to_string()))
    }

    /// Get space child events for a room
    pub async fn get_space_child_events(
        &self,
        room_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT state_key, content
            FROM event
            WHERE room_id = $room_id
            AND type = 'm.space.child'
            AND state_key != ''
            ORDER BY depth DESC, origin_server_ts DESC
        ";

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get room state events by specific types
    pub async fn get_room_state_by_types(
        &self,
        room_id: &str,
        event_types: &[&str],
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT type, state_key, content
            FROM event
            WHERE room_id = $room_id
            AND type IN $event_types
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_types", event_types.iter().map(|s| s.to_string()).collect::<Vec<String>>()))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get room state events by specific event IDs for federation
    pub async fn get_room_state_events_by_ids(&self, room_id: &str, event_ids: &[String]) -> Result<Vec<Event>, RepositoryError> {
        if event_ids.is_empty() {
            return Ok(Vec::new());
        }

        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND event_id IN $event_ids 
            AND state_key IS NOT NULL
            ORDER BY origin_server_ts ASC
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_ids", event_ids.iter().map(|s| s.to_string()).collect::<Vec<String>>()))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get room state event IDs only for federation state_ids endpoint
    pub async fn get_room_state_event_ids(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT event_id FROM event 
            WHERE room_id = $room_id 
            AND state_key IS NOT NULL
            ORDER BY origin_server_ts ASC
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let event_ids: Vec<String> = response.take(0)?;
        Ok(event_ids)
    }

    /// Get auth event IDs in batches for federation
    pub async fn get_auth_event_ids_batch(&self, auth_event_ids: &[String]) -> Result<Vec<String>, RepositoryError> {
        if auth_event_ids.is_empty() {
            return Ok(Vec::new());
        }

        let query = "
            SELECT event_id FROM event 
            WHERE event_id IN $auth_event_ids
            ORDER BY origin_server_ts ASC
        ";

        let mut response = self.db.query(query)
            .bind(("auth_event_ids", auth_event_ids.iter().map(|s| s.to_string()).collect::<Vec<String>>()))
            .await?;

        let event_ids: Vec<String> = response.take(0)?;
        Ok(event_ids)
    }

    /// Get room backfill events with limit and direction for federation
    pub async fn get_room_backfill_events(&self, room_id: &str, limit: u32, from_token: Option<&str>) -> Result<Vec<Event>, RepositoryError> {
        let query = if let Some(_token) = from_token {
            "
                SELECT * FROM event 
                WHERE room_id = $room_id 
                AND origin_server_ts < $from_token
                ORDER BY origin_server_ts DESC
                LIMIT $limit
            "
        } else {
            "
                SELECT * FROM event 
                WHERE room_id = $room_id 
                ORDER BY origin_server_ts DESC
                LIMIT $limit
            "
        };

        let mut response = if let Some(token) = from_token {
            self.db.query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("from_token", token.to_string()))
                .bind(("limit", limit as i64))
                .await?
        } else {
            self.db.query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("limit", limit as i64))
                .await?
        };

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get missing events batch for federation
    pub async fn get_missing_events_batch(&self, room_id: &str, earliest_events: &[String], latest_events: &[String], limit: u32) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND event_id NOT IN $earliest_events
            AND event_id NOT IN $latest_events
            ORDER BY origin_server_ts ASC
            LIMIT $limit
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("earliest_events", earliest_events.iter().map(|s| s.to_string()).collect::<Vec<String>>()))
            .bind(("latest_events", latest_events.iter().map(|s| s.to_string()).collect::<Vec<String>>()))
            .bind(("limit", limit as i64))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get event by ID with room validation for federation
    pub async fn get_event_by_id_with_room(&self, event_id: &str, room_id: &str) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
            WHERE event_id = $event_id 
            AND room_id = $room_id
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events.into_iter().next())
    }

    /// Get room state by specific type and key for client endpoints
    pub async fn get_room_state_by_type_and_key(&self, room_id: &str, event_type: &str, state_key: &str) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND event_type = $event_type 
            AND state_key = $state_key
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_type", event_type.to_string()))
            .bind(("state_key", state_key.to_string()))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events.into_iter().next())
    }

    /// Get room state by specific type for client endpoints
    pub async fn get_room_state_by_type(&self, room_id: &str, event_type: &str) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND event_type = $event_type 
            AND state_key IS NOT NULL
            ORDER BY origin_server_ts DESC
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_type", event_type.to_string()))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get all room state events for client endpoints
    pub async fn get_room_state_events(&self, room_id: &str) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND state_key IS NOT NULL
            ORDER BY origin_server_ts DESC
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Validate state access for client endpoints
    pub async fn validate_state_access(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        // Check if user is a member of the room
        let query = "
            SELECT COUNT() as count FROM membership 
            WHERE room_id = $room_id 
            AND user_id = $user_id 
            AND membership IN ['join', 'invite']
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let counts: Vec<serde_json::Value> = response.take(0)?;
        if let Some(count_obj) = counts.first()
            && let Some(count) = count_obj.get("count").and_then(|v| v.as_u64()) {
                return Ok(count > 0);
            }
        
        Ok(false)
    }
}

/// Validate state event content based on event type
fn validate_state_event_content(
    event_type: &str,
    content: &serde_json::Value
) -> Result<(), RepositoryError> {
    match event_type {
        "m.room.name" => {
            if let Some(name) = content.get("name") {
                if !name.is_string() {
                    return Err(RepositoryError::Validation {
                        field: "name".to_string(),
                        message: "m.room.name content.name must be a string".to_string(),
                    });
                }
            }
        },
        "m.room.topic" => {
            if let Some(topic) = content.get("topic") {
                if !topic.is_string() {
                    return Err(RepositoryError::Validation {
                        field: "topic".to_string(),
                        message: "m.room.topic content.topic must be a string".to_string(),
                    });
                }
            }
        },
        "m.room.avatar" => {
            if let Some(url) = content.get("url") {
                if !url.is_string() {
                    return Err(RepositoryError::Validation {
                        field: "url".to_string(),
                        message: "m.room.avatar content.url must be a string".to_string(),
                    });
                }
            }
        },
        "m.room.canonical_alias" => {
            if let Some(alias) = content.get("alias") {
                if !alias.is_string() && !alias.is_null() {
                    return Err(RepositoryError::Validation {
                        field: "alias".to_string(),
                        message: "m.room.canonical_alias content.alias must be a string or null".to_string(),
                    });
                }
            }
        },
        _ => {
            // Other event types: no validation
        },
    }
    Ok(())
}

impl EventRepository {
    /// Update room state event for client endpoints
    pub async fn update_room_state_event(&self, room_id: &str, event_type: &str, state_key: &str, content: serde_json::Value, sender: &str, server_name: &str) -> Result<Event, RepositoryError> {
        // Validate content based on event type
        validate_state_event_content(event_type, &content)?;

        // Check authorization
        let power_repo = PowerLevelsRepository::new(self.db.clone());
        let can_send = power_repo
            .can_user_perform_action(
                room_id,
                sender,
                PowerLevelAction::SendState(event_type.to_string())
            )
            .await?;

        if !can_send {
            return Err(RepositoryError::Forbidden {
                reason: format!(
                    "User {} does not have permission to send {} state events in room {}",
                    sender, event_type, room_id
                ),
            });
        }

        // Generate event ID with proper server name
        let event_id = format!("${}:{}", uuid::Uuid::new_v4(), server_name);
        let now = chrono::Utc::now();

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: sender.to_string(),
            event_type: event_type.to_string(),
            content: matryx_entity::EventContent::Unknown(content),
            state_key: Some(state_key.to_string()),
            origin_server_ts: now.timestamp_millis(),
            unsigned: None,
            prev_events: Some(Vec::new()),
            auth_events: Some(Vec::new()),
            depth: Some(1),
            hashes: Some(std::collections::HashMap::new()),
            signatures: Some(std::collections::HashMap::new()),
            redacts: None,
            outlier: Some(false),
            received_ts: Some(now.timestamp_millis()),
            rejected_reason: None,
            soft_failed: None,
        };

        let created_event: Option<Event> = self.db.create(("event", &event_id)).content(event).await?;
        created_event.ok_or_else(|| RepositoryError::NotFound { 
            entity_type: "event".to_string(), 
            id: event_id 
        })
    }

    /// Get next event depth for a room
    pub async fn get_next_event_depth(&self, room_id: &str) -> Result<i64, RepositoryError> {
        let query = "SELECT VALUE math::max(depth) FROM event WHERE room_id = $room_id";
        let mut result = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let max_depth: Option<i64> = result.take(0)?;
        Ok(max_depth.unwrap_or(0) + 1)
    }

    /// Get latest event IDs for a room
    pub async fn get_latest_event_ids(&self, room_id: &str, limit: u32) -> Result<Vec<String>, RepositoryError> {
        let query = format!(
            "SELECT event_id FROM event WHERE room_id = $room_id ORDER BY depth DESC LIMIT {}",
            limit
        );
        let mut result = self.db.query(&query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let events: Vec<serde_json::Value> = result.take(0)?;
        let event_ids: Vec<String> = events
            .into_iter()
            .filter_map(|v| v.get("event_id").and_then(|id| id.as_str().map(|s| s.to_string())))
            .collect();

        Ok(event_ids)
    }

    /// Get auth events for unban operation
    pub async fn get_auth_events_for_unban(&self, room_id: &str, unbanner_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT event_id FROM event 
            WHERE room_id = $room_id 
            AND ((event_type = 'm.room.create' AND state_key = '')
                 OR (event_type = 'm.room.power_levels' AND state_key = '')
                 OR (event_type = 'm.room.member' AND state_key = $unbanner_id))
            ORDER BY origin_server_ts ASC
        ";

        let mut result = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("unbanner_id", unbanner_id.to_string()))
            .await?;

        let events: Vec<serde_json::Value> = result.take(0)?;
        let auth_event_ids: Vec<String> = events
            .into_iter()
            .filter_map(|v| v.get("event_id").and_then(|id| id.as_str().map(|s| s.to_string())))
            .collect();

        Ok(auth_event_ids)
    }

    /// Check if a room is a direct message room
    pub async fn is_direct_message_room(&self, room_id: &str) -> Result<bool, RepositoryError> {
        // Check member count
        let member_count_query = "
            SELECT count() as count
            FROM membership 
            WHERE room_id = $room_id 
              AND membership = 'join'
        ";

        let mut response = self.db.query(member_count_query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let member_count_result: Option<CountResult> = response.take(0)?;
        let member_count = member_count_result.map(|c| c.count).unwrap_or(0);

        // Check for room name or topic
        let room_state_query = "
            SELECT count() as count
            FROM event 
            WHERE room_id = $room_id 
              AND event_type IN ['m.room.name', 'm.room.topic']
              AND state_key = ''
        ";

        let mut response = self.db.query(room_state_query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let has_name_or_topic_result: Option<CountResult> = response.take(0)?;
        let has_name_or_topic = has_name_or_topic_result.map(|c| c.count > 0).unwrap_or(false);

        Ok(member_count == 2 && !has_name_or_topic)
    }

    /// Check authorization rules for knock event
    pub async fn check_knock_authorization(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        // For knock events, the main authorization check is that the room allows knocking
        // and the user is not banned or already in the room (checked earlier)

        // Check if user has any power level restrictions
        let query = "
            SELECT content.users
            FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.power_levels' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct PowerLevelsContent {
            users: Option<serde_json::Map<String, serde_json::Value>>,
        }

        let power_levels: Option<PowerLevelsContent> = response.take(0)?;

        if let Some(levels) = power_levels
            && let Some(users) = levels.users {
                // Check if user has negative power level (effectively banned from actions)
                if let Some(user_level) = users.get(user_id).and_then(|v| v.as_i64())
                    && user_level < 0 {
                        return Ok(false);
                    }
            }

        Ok(true)
    }

    /// Get room state events to include in knock response
    pub async fn get_room_state_for_knock(&self, room_id: &str) -> Result<Vec<serde_json::Value>, RepositoryError> {
        // Return essential room state events for the knocking user
        let query = "
            SELECT *
            FROM event 
            WHERE room_id = $room_id 
            AND state_key IS NOT NULL
            AND event_type IN [
                'm.room.create',
                'm.room.join_rules', 
                'm.room.power_levels',
                'm.room.name',
                'm.room.topic',
                'm.room.avatar',
                'm.room.canonical_alias'
            ]
            ORDER BY depth DESC, origin_server_ts DESC
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let events: Vec<Event> = response.take(0)?;

        // Convert events to JSON format for response
        let state_events: Vec<serde_json::Value> = events
            .into_iter()
            .map(|event| serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({})))
            .collect();

        Ok(state_events)
    }

    /// Get current state event by type and state_key
    pub async fn get_current_state_event(&self, room_id: &str, event_type: &str, state_key: &str) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT *
            FROM event
            WHERE room_id = $room_id
            AND event_type = $event_type
            AND state_key = $state_key
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_type", event_type.to_string()))
            .bind(("state_key", state_key.to_string()))
            .await?;

        let event: Option<Event> = response.take(0)?;
        Ok(event)
    }

    /// Get room power levels content
    pub async fn get_room_power_levels(&self, room_id: &str) -> Result<serde_json::Value, RepositoryError> {
        let query = "
            SELECT content
            FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.power_levels'
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct PowerLevelsEvent {
            content: serde_json::Value,
        }

        let power_levels: Option<PowerLevelsEvent> = response.take(0)?;
        Ok(power_levels.map(|p| p.content).unwrap_or_else(|| serde_json::json!({})))
    }

    /// Get current depth for a room
    pub async fn get_room_current_depth(&self, room_id: &str) -> Result<i64, RepositoryError> {
        let query = "
            SELECT depth
            FROM event
            WHERE room_id = $room_id
            ORDER BY depth DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct DepthResult {
            depth: i64,
        }

        let depth: Option<DepthResult> = response.take(0)?;
        Ok(depth.map(|d| d.depth).unwrap_or(0))
    }

    /// Get room state event IDs at a specific event
    pub async fn get_room_state_ids_at_event(&self, room_id: &str, event_id: &str) -> Result<Vec<String>, RepositoryError> {
        // Get the target event's depth for state resolution
        let depth_query = "
            SELECT depth
            FROM event
            WHERE event_id = $event_id
        ";

        let mut response = self.db.query(depth_query)
            .bind(("event_id", event_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct EventDepth {
            depth: i64,
        }

        let event_depth: Option<EventDepth> = response.take(0)?;
        let target_depth = event_depth.map(|e| e.depth).ok_or_else(|| RepositoryError::NotFound {
            entity_type: "event_depth".to_string(),
            id: event_id.to_string(),
        })?;

        // Get state event IDs at or before the target event depth
        let state_query = "
            SELECT event_id
            FROM event
            WHERE room_id = $room_id
            AND state_key IS NOT NULL
            AND depth <= $target_depth
            AND (
                SELECT COUNT()
                FROM event e2
                WHERE e2.room_id = $room_id
                AND e2.event_type = event.event_type
                AND e2.state_key = event.state_key
                AND e2.depth <= $target_depth
                AND (e2.depth > event.depth OR (e2.depth = event.depth AND e2.origin_server_ts > event.origin_server_ts))
            ) = 0
            ORDER BY event_type, state_key
        ";

        let mut response = self.db.query(state_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("target_depth", target_depth))
            .await?;

        #[derive(serde::Deserialize)]
        struct EventId {
            event_id: String,
        }

        let events: Vec<EventId> = response.take(0)?;
        let state_event_ids: Vec<String> = events.into_iter().map(|e| e.event_id).collect();

        Ok(state_event_ids)
    }

    /// Get auth chain IDs for a set of state event IDs
    pub async fn get_auth_chain_ids_for_state(&self, state_event_ids: &[String]) -> Result<Vec<String>, RepositoryError> {
        if state_event_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut auth_event_ids = std::collections::HashSet::new();
        let mut to_process = std::collections::HashSet::new();

        // Clone the state_event_ids to avoid lifetime issues
        let state_event_ids_owned: Vec<String> = state_event_ids.to_vec();

        // Get initial auth events from state events
        let query = "
            SELECT auth_events
            FROM event
            WHERE event_id IN $state_event_ids
        ";

        let mut response = self.db.query(query)
            .bind(("state_event_ids", state_event_ids_owned))
            .await?;

        #[derive(serde::Deserialize)]
        struct AuthEvents {
            auth_events: Option<Vec<String>>,
        }

        let events: Vec<AuthEvents> = response.take(0)?;

        // Collect initial auth event IDs
        for event in events {
            if let Some(auth_events) = event.auth_events {
                for auth_event_id in auth_events {
                    if auth_event_ids.insert(auth_event_id.clone()) {
                        to_process.insert(auth_event_id);
                    }
                }
            }
        }

        // Recursively fetch auth events
        while !to_process.is_empty() {
            let current_batch: Vec<String> = to_process.drain().collect();

            let query = "
                SELECT auth_events
                FROM event
                WHERE event_id IN $auth_event_ids
            ";

            let mut response = self.db.query(query)
                .bind(("auth_event_ids", current_batch))
                .await?;

            let events: Vec<AuthEvents> = response.take(0)?;

            // Process auth events of the fetched events
            for event in events {
                if let Some(auth_events) = event.auth_events {
                    for auth_event_id in auth_events {
                        if auth_event_ids.insert(auth_event_id.clone()) {
                            to_process.insert(auth_event_id);
                        }
                    }
                }
            }
        }

        let auth_chain_ids: Vec<String> = auth_event_ids.into_iter().collect();
        Ok(auth_chain_ids)
    }

    /// Get the current state of a room with optional event exclusion
    pub async fn get_room_current_state(&self, room_id: &str, exclude_event_id: Option<&str>) -> Result<Vec<Event>, RepositoryError> {
        let mut query = "
            SELECT *
            FROM event
            WHERE room_id = $room_id
            AND state_key IS NOT NULL
            AND (
                SELECT COUNT() 
                FROM event e2 
                WHERE e2.room_id = $room_id 
                AND e2.event_type = event.event_type 
                AND e2.state_key = event.state_key 
                AND (e2.depth > event.depth OR (e2.depth = event.depth AND e2.origin_server_ts > event.origin_server_ts))
            ) = 0
            ORDER BY event_type, state_key
        ".to_string();

        let mut response = if let Some(exclude_id) = exclude_event_id {
            query = format!("{} AND event_id != $exclude_event_id", query);
            self.db.query(&query)
                .bind(("room_id", room_id.to_string()))
                .bind(("exclude_event_id", exclude_id.to_string()))
                .await?
        } else {
            self.db.query(&query)
                .bind(("room_id", room_id.to_string()))
                .await?
        };

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get the auth chain for a set of events
    pub async fn get_auth_chain_for_events(&self, events: &[Event]) -> Result<Vec<Event>, RepositoryError> {
        let mut auth_event_ids = std::collections::HashSet::new();

        // Collect all auth_events from the provided events
        for event in events {
            if let Some(auth_events) = &event.auth_events {
                for auth_event_id in auth_events {
                    auth_event_ids.insert(auth_event_id.clone());
                }
            }
        }

        if auth_event_ids.is_empty() {
            return Ok(vec![]);
        }

        // Convert HashSet to Vec for query binding
        let auth_ids: Vec<String> = auth_event_ids.into_iter().collect();

        let query = "
            SELECT *
            FROM event
            WHERE event_id IN $auth_event_ids
            ORDER BY depth, origin_server_ts
        ";

        let mut response = self.db.query(query)
            .bind(("auth_event_ids", auth_ids))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }

    /// Get server signing key for event signing
    pub async fn get_server_signing_key(
        &self,
        server_name: &str,
    ) -> Result<Option<ServerSigningKey>, RepositoryError> {
        let query = "
            SELECT private_key, key_id 
            FROM server_signing_keys 
            WHERE server_name = $server_name 
              AND is_active = true 
            ORDER BY created_at DESC 
            LIMIT 1
        ";

        let mut response = self.db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .await?;

        let key_records: Vec<ServerSigningKey> = response.take(0)?;
        Ok(key_records.into_iter().next())
    }

    /// Get events by IDs for backfill
    pub async fn get_events_by_ids_for_backfill(
        &self,
        event_ids: &[String],
        room_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = "
        SELECT *
        FROM event
        WHERE event_id IN $event_ids
        AND room_id = $room_id
        ORDER BY depth DESC, origin_server_ts DESC
    ";

    let mut response = self.db
        .query(query)
        .bind(("event_ids", event_ids.to_vec()))
        .bind(("room_id", room_id.to_string()))
        .await?;

    let events: Vec<Event> = response.take(0)?;
    Ok(events)
    }

    /// Get server ACL event for a room
    pub async fn get_server_acl_event(
        &self,
        room_id: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        let query = "
        SELECT * FROM event
        WHERE room_id = $room_id
        AND event_type = 'm.room.server_acl'
        AND state_key = ''
        ORDER BY depth DESC, origin_server_ts DESC
        LIMIT 1
    ";

    let mut response = self.db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events.into_iter().next())
    }

    /// Get events by IDs with minimum depth for missing events
    pub async fn get_events_by_ids_with_min_depth(
        &self,
        event_ids: &[String],
        room_id: &str,
        min_depth: i64,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT *
            FROM event
            WHERE event_id IN $event_ids
            AND room_id = $room_id
            AND depth >= $min_depth
            ORDER BY depth DESC, origin_server_ts DESC
        ";

        let mut response = self.db
            .query(query)
            .bind(("event_ids", event_ids.to_vec()))
            .bind(("room_id", room_id.to_string()))
            .bind(("min_depth", min_depth))
            .await?;

        let events: Vec<Event> = response.take(0)?;
        Ok(events)
    }
}

#[derive(serde::Deserialize)]
pub struct ServerSigningKey {
    pub private_key: String,
    pub key_id: String,
}
