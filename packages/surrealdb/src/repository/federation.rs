use crate::repository::error::RepositoryError;
use base64::{Engine, engine::general_purpose};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use matryx_entity::types::Event;
use matryx_entity::utils::canonical_json::canonical_json_for_signing;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use surrealdb::{Surreal, engine::any::Any};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomHierarchy {
    pub room_id: String,
    pub children: Vec<HierarchyChild>,
    pub children_state: Vec<Event>,
    pub inaccessible_children: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyChild {
    pub room_id: String,
    pub via: Vec<String>,
    pub room_type: Option<String>,
    pub suggested: bool,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKey {
    pub server_name: String,
    pub key_id: String,
    pub verify_key: String,
    pub valid_until_ts: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationValidationResult {
    pub valid: bool,
    pub reason: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DirectToDeviceEduParams<'a> {
    pub message_id: &'a str,
    pub origin: &'a str,
    pub sender: &'a str,
    pub message_type: &'a str,
    pub content: serde_json::Value,
    pub target_user_id: &'a str,
    pub target_device_id: Option<&'a str>,
}

pub struct ProcessDeviceListEduParams {
    pub user_id: String,
    pub device_id: String,
    pub stream_id: i64,
    pub deleted: bool,
    pub prev_id: Option<Vec<i64>>,
    pub device_display_name: Option<String>,
    pub keys: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationSettings {
    pub federate: bool,
    pub restricted_servers: Vec<String>,
    pub allowed_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureValidation {
    pub valid: bool,
    pub verified_signatures: HashMap<String, HashMap<String, bool>>,
    pub missing_signatures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinResult {
    pub event_id: String,
    pub state: Vec<Event>,
    pub auth_chain: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveResult {
    pub event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockResult {
    pub event_id: String,
    pub knock_state: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateIdsResponse {
    pub pdu_ids: Vec<String>,
    pub auth_chain_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillResponse {
    pub pdus: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyInvite {
    pub display_name: String,
    pub key_validity_url: String,
    pub public_key: String,
    pub public_keys: Vec<serde_json::Value>,
}

pub struct FederationRepository {
    pub db: Surreal<Any>,
}

impl FederationRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub fn get_db(&self) -> &Surreal<Any> {
        &self.db
    }

    /// Get room hierarchy for federation
    pub async fn get_room_hierarchy(
        &self,
        room_id: &str,
        suggested_only: bool,
    ) -> Result<RoomHierarchy, RepositoryError> {
        // Get room's child relationships
        let child_query = if suggested_only {
            "
            SELECT * FROM room_hierarchy 
            WHERE parent_room_id = $room_id AND suggested = true
            ORDER BY order ASC
            "
        } else {
            "
            SELECT * FROM room_hierarchy 
            WHERE parent_room_id = $room_id
            ORDER BY order ASC
            "
        };

        let mut result = self.db.query(child_query).bind(("room_id", room_id.to_string())).await?;
        let children_data: Vec<serde_json::Value> = result.take(0)?;

        let mut children = Vec::new();
        let mut inaccessible_children = Vec::new();

        for child_data in children_data {
            if let Some(child_room_id) = child_data.get("child_room_id").and_then(|v| v.as_str()) {
                // Check if child room is accessible
                let accessible_query = "
                    SELECT room_id FROM room 
                    WHERE room_id = $child_room_id AND is_public = true
                    LIMIT 1
                ";
                let mut access_result = self
                    .db
                    .query(accessible_query)
                    .bind(("child_room_id", child_room_id.to_string()))
                    .await?;
                let accessible: Vec<serde_json::Value> = access_result.take(0)?;

                if accessible.is_empty() {
                    inaccessible_children.push(child_room_id.to_string());
                } else {
                    children.push(HierarchyChild {
                        room_id: child_room_id.to_string(),
                        via: child_data
                            .get("via")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        room_type: child_data
                            .get("room_type")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        suggested: child_data
                            .get("suggested")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        order: child_data
                            .get("order")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    });
                }
            }
        }

        // Get children state events
        let state_query = "
            SELECT * FROM event 
            WHERE room_id IN $child_room_ids 
            AND event_type IN ['m.room.name', 'm.room.topic', 'm.room.avatar', 'm.room.canonical_alias']
            AND state_key = ''
        ";
        let child_room_ids: Vec<String> = children.iter().map(|c| c.room_id.clone()).collect();
        let mut state_result =
            self.db.query(state_query).bind(("child_room_ids", child_room_ids)).await?;
        let children_state: Vec<Event> = state_result.take(0)?;

        Ok(RoomHierarchy {
            room_id: room_id.to_string(),
            children,
            children_state,
            inaccessible_children,
        })
    }

    /// Validate federation request
    pub async fn validate_federation_request(
        &self,
        origin: &str,
        destination: &str,
        request_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if request ID has been seen before (replay protection)
        let query = "
            SELECT count() FROM federation_request_log 
            WHERE origin = $origin AND request_id = $request_id
            AND created_at > $cutoff
            GROUP ALL
        ";
        let cutoff = Utc::now() - chrono::Duration::hours(1); // 1 hour window

        let mut result = self
            .db
            .query(query)
            .bind(("origin", origin.to_string()))
            .bind(("request_id", request_id.to_string()))
            .bind(("cutoff", cutoff))
            .await?;
        let count: Option<i64> = result.take(0)?;

        if count.unwrap_or(0) > 0 {
            return Ok(false); // Request already seen
        }

        // Log the request
        let log_query = "
            CREATE federation_request_log SET 
            origin = $origin,
            destination = $destination,
            request_id = $request_id,
            created_at = $created_at
        ";
        self.db
            .query(log_query)
            .bind(("origin", origin.to_string()))
            .bind(("destination", destination.to_string()))
            .bind(("request_id", request_id.to_string()))
            .bind(("created_at", Utc::now()))
            .await?;

        Ok(true)
    }

    /// Get server keys for federation
    pub async fn get_server_keys(
        &self,
        server_name: &str,
    ) -> Result<Vec<ServerKey>, RepositoryError> {
        let query = "
            SELECT * FROM server_keys 
            WHERE server_name = $server_name 
            AND (valid_until_ts IS NULL OR valid_until_ts > $now)
            ORDER BY created_at DESC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("now", Utc::now()))
            .await?;
        let keys: Vec<ServerKey> = result.take(0)?;
        Ok(keys)
    }

    /// Store server keys
    pub async fn store_server_keys(
        &self,
        server_name: &str,
        keys: &[ServerKey],
    ) -> Result<(), RepositoryError> {
        for key in keys {
            let query = "
                CREATE server_keys SET
                server_name = $server_name,
                key_id = $key_id,
                verify_key = $verify_key,
                valid_until_ts = $valid_until_ts,
                created_at = $created_at
            ";
            self.db
                .query(query)
                .bind(("server_name", server_name.to_string()))
                .bind(("key_id", key.key_id.clone()))
                .bind(("verify_key", key.verify_key.clone()))
                .bind(("valid_until_ts", key.valid_until_ts))
                .bind(("created_at", key.created_at))
                .await?;
        }
        Ok(())
    }

    /// Validate event signature for federation
    pub async fn validate_event_signature(
        &self,
        event: &Event,
        origin: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if event has signatures from origin server
        let signatures = match &event.signatures {
            Some(sigs) => sigs,
            None => return Ok(false),
        };

        let origin_sigs = match signatures.get(origin) {
            Some(sigs) => sigs,
            None => return Ok(false),
        };

        // Get all keys for the origin server
        let server_keys = match self.get_server_keys(origin).await {
            Ok(keys) => keys,
            Err(_) => return Ok(false),
        };

        // Try each signature until one verifies
        for (key_id, signature_b64) in origin_sigs {
            // Find the matching public key
            let public_key = match server_keys.iter().find(|k| &k.key_id == key_id) {
                Some(key) => &key.verify_key,
                None => continue,
            };

            // Decode signature and public key from base64
            let sig_bytes = match general_purpose::STANDARD.decode(signature_b64) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let key_bytes = match general_purpose::STANDARD.decode(public_key) {
                Ok(b) => b,
                Err(_) => continue,
            };

            // Verify signature
            if sig_bytes.len() == 64 && key_bytes.len() == 32 {
                let key_array: [u8; 32] = match key_bytes.try_into() {
                    Ok(arr) => arr,
                    Err(_) => continue,
                };
                let sig_array: [u8; 64] = match sig_bytes.try_into() {
                    Ok(arr) => arr,
                    Err(_) => continue,
                };

                match VerifyingKey::from_bytes(&key_array) {
                    Ok(verifying_key) => {
                        let signature_obj = Signature::from_bytes(&sig_array);

                        // Get canonical JSON of event
                        let event_value = match serde_json::to_value(event) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        let canonical = match canonical_json_for_signing(&event_value) {
                            Ok(c) => c,
                            Err(_) => continue,
                        };

                        if verifying_key.verify(canonical.as_bytes(), &signature_obj).is_ok() {
                            return Ok(true); // Valid signature found
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        Ok(false) // No valid signatures found
    }

    /// Get room state for federation
    pub async fn get_room_state_for_federation(
        &self,
        room_id: &str,
        event_id: Option<&str>,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = if event_id.is_some() {
            "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND state_key IS NOT NULL
            AND origin_server_ts <= (
                SELECT origin_server_ts FROM event WHERE event_id = $event_id
            )
            ORDER BY origin_server_ts DESC
            "
        } else {
            "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND state_key IS NOT NULL
            ORDER BY origin_server_ts DESC
            "
        };

        let mut result = if let Some(event_id) = event_id {
            self.db
                .query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("event_id", event_id.to_string()))
                .await?
        } else {
            self.db.query(query).bind(("room_id", room_id.to_string())).await?
        };

        let events: Vec<Event> = result.take(0)?;
        Ok(events)
    }

    /// Validate PDU for federation
    pub async fn validate_pdu(
        &self,
        pdu: &Event,
        origin: &str,
    ) -> Result<FederationValidationResult, RepositoryError> {
        // Basic validation checks

        // Check if event is from the claimed origin
        if !pdu.sender.ends_with(&format!(":{}", origin)) {
            return Ok(FederationValidationResult {
                valid: false,
                reason: Some("Event sender does not match origin server".to_string()),
                error_code: Some("M_FORBIDDEN".to_string()),
            });
        }

        // Check if room exists and is federated
        let room_query = "
            SELECT federate FROM room 
            WHERE room_id = $room_id
            LIMIT 1
        ";
        let mut result = self.db.query(room_query).bind(("room_id", pdu.room_id.clone())).await?;
        let room_data: Vec<serde_json::Value> = result.take(0)?;

        if room_data.is_empty() {
            return Ok(FederationValidationResult {
                valid: false,
                reason: Some("Room not found".to_string()),
                error_code: Some("M_NOT_FOUND".to_string()),
            });
        }

        let federate = room_data[0].get("federate").and_then(|v| v.as_bool()).unwrap_or(true);
        if !federate {
            return Ok(FederationValidationResult {
                valid: false,
                reason: Some("Room does not allow federation".to_string()),
                error_code: Some("M_FORBIDDEN".to_string()),
            });
        }

        // Validate event signature
        if !self.validate_event_signature(pdu, origin).await? {
            return Ok(FederationValidationResult {
                valid: false,
                reason: Some("Invalid event signature".to_string()),
                error_code: Some("M_FORBIDDEN".to_string()),
            });
        }

        Ok(FederationValidationResult { valid: true, reason: None, error_code: None })
    }

    /// Check federation ACL for room
    pub async fn check_federation_acl(
        &self,
        room_id: &str,
        server_name: &str,
    ) -> Result<bool, RepositoryError> {
        // Get room's federation ACL settings
        let query = "
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.server_acl'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let acl_events: Vec<serde_json::Value> = result.take(0)?;

        if acl_events.is_empty() {
            return Ok(true); // No ACL means all servers allowed
        }

        if let Some(content) = acl_events[0].get("content") {
            // Check deny list
            if let Some(deny_list) = content.get("deny").and_then(|v| v.as_array()) {
                for pattern in deny_list {
                    if let Some(pattern_str) = pattern.as_str()
                        && self.matches_server_pattern(server_name, pattern_str) {
                        return Ok(false);
                    }
                }
            }

            // Check allow list
            if let Some(allow_list) = content.get("allow").and_then(|v| v.as_array()) {
                for pattern in allow_list {
                    if let Some(pattern_str) = pattern.as_str()
                        && self.matches_server_pattern(server_name, pattern_str) {
                        return Ok(true);
                    }
                }
                return Ok(false); // Allow list exists but server not in it
            }
        }

        Ok(true) // Default allow
    }

    /// Helper method to match server patterns (supports wildcards)
    fn matches_server_pattern(&self, server_name: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if let Some(domain) = pattern.strip_prefix("*.") {
            return server_name.ends_with(domain) || server_name == &domain[1..];
        }

        server_name == pattern
    }

    /// Get federation statistics
    pub async fn get_federation_stats(
        &self,
        server_name: &str,
    ) -> Result<serde_json::Value, RepositoryError> {
        let query = "
            SELECT 
                count() as total_requests,
                count(CASE WHEN created_at > $recent_cutoff THEN 1 END) as recent_requests
            FROM federation_request_log 
            WHERE origin = $server_name OR destination = $server_name
            GROUP ALL
        ";
        let recent_cutoff = Utc::now() - chrono::Duration::hours(24);

        let mut result = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("recent_cutoff", recent_cutoff))
            .await?;
        let stats: Vec<serde_json::Value> = result.take(0)?;

        Ok(stats.into_iter().next().unwrap_or(serde_json::json!({})))
    }

    // Federation management operations

    /// Make join event for federation
    pub async fn make_join_event(
        &self,
        room_id: &str,
        user_id: &str,
        room_version: &str,
    ) -> Result<Event, RepositoryError> {
        // Event ID format depends on room version
        let event_id = if room_version >= "4" {
            format!("${}", uuid::Uuid::new_v4())
        } else {
            format!("!{}", uuid::Uuid::new_v4())
        };
        let now = chrono::Utc::now();

        let content = serde_json::json!({
            "membership": "join",
            "displayname": null,
            "avatar_url": null
        });

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: user_id.to_string(),
            event_type: "m.room.member".to_string(),
            content: matryx_entity::types::EventContent::Unknown(content),
            state_key: Some(user_id.to_string()),
            origin_server_ts: now.timestamp_millis(),
            unsigned: None,
            prev_events: None, // Will be populated by caller
            auth_events: None, // Will be populated by caller
            depth: None,       // Will be populated by caller
            hashes: None,
            signatures: None,
            redacts: None,
            outlier: Some(false),
            received_ts: Some(now.timestamp_millis()),
            rejected_reason: None,
            soft_failed: Some(false),
        };

        Ok(event)
    }

    /// Make leave event for federation
    pub async fn make_leave_event(
        &self,
        room_id: &str,
        user_id: &str,
        room_version: &str,
    ) -> Result<Event, RepositoryError> {
        // Event ID format depends on room version
        let event_id = if room_version >= "4" {
            format!("${}", uuid::Uuid::new_v4())
        } else {
            format!("!{}", uuid::Uuid::new_v4())
        };
        let now = chrono::Utc::now();

        let content = serde_json::json!({
            "membership": "leave"
        });

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: user_id.to_string(),
            event_type: "m.room.member".to_string(),
            content: matryx_entity::types::EventContent::Unknown(content),
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

        Ok(event)
    }

    /// Make knock event for federation
    pub async fn make_knock_event(
        &self,
        room_id: &str,
        user_id: &str,
        room_version: &str,
    ) -> Result<Event, RepositoryError> {
        // Event ID format depends on room version
        let event_id = if room_version >= "4" {
            format!("${}", uuid::Uuid::new_v4())
        } else {
            format!("!{}", uuid::Uuid::new_v4())
        };
        let now = chrono::Utc::now();

        let content = serde_json::json!({
            "membership": "knock",
            "reason": "Requesting to join room"
        });

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: user_id.to_string(),
            event_type: "m.room.member".to_string(),
            content: matryx_entity::types::EventContent::Unknown(content),
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

        Ok(event)
    }

    /// Process join event from federation
    pub async fn process_join_event(
        &self,
        room_id: &str,
        event: &Event,
        origin: &str,
    ) -> Result<JoinResult, RepositoryError> {
        // Validate the join event
        let validation = self.validate_pdu(event, origin).await?;
        if !validation.valid {
            return Err(RepositoryError::Validation {
                field: "event".to_string(),
                message: validation.reason.unwrap_or("Invalid join event".to_string()),
            });
        }

        // Get current room state
        let state = self.get_room_state_for_federation(room_id, None).await?;

        // Get auth chain for the event
        let auth_chain_query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND event_type IN ['m.room.create', 'm.room.join_rules', 'm.room.power_levels']
            AND state_key = ''
        ";
        let mut result = self
            .db
            .query(auth_chain_query)
            .bind(("room_id", room_id.to_string()))
            .await?;
        let auth_chain: Vec<Event> = result.take(0)?;

        Ok(JoinResult {
            event_id: event.event_id.clone(),
            state,
            auth_chain,
        })
    }

    /// Process leave event from federation
    pub async fn process_leave_event(
        &self,
        room_id: &str,
        event: &Event,
        origin: &str,
    ) -> Result<LeaveResult, RepositoryError> {
        // Validate that event belongs to the specified room
        if event.room_id != room_id {
            return Err(RepositoryError::Validation {
                field: "room_id".to_string(),
                message: "Event room_id does not match specified room".to_string(),
            });
        }

        // Validate the leave event
        let validation = self.validate_pdu(event, origin).await?;
        if !validation.valid {
            return Err(RepositoryError::Validation {
                field: "event".to_string(),
                message: validation.reason.unwrap_or("Invalid leave event".to_string()),
            });
        }

        Ok(LeaveResult { event_id: event.event_id.clone() })
    }

    /// Process knock event from federation
    pub async fn process_knock_event(
        &self,
        room_id: &str,
        event: &Event,
        origin: &str,
    ) -> Result<KnockResult, RepositoryError> {
        // Validate the knock event
        let validation = self.validate_pdu(event, origin).await?;
        if !validation.valid {
            return Err(RepositoryError::Validation {
                field: "event".to_string(),
                message: validation.reason.unwrap_or("Invalid knock event".to_string()),
            });
        }

        // Get knock state (subset of room state relevant for knocking)
        let knock_state_query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND event_type IN ['m.room.create', 'm.room.join_rules', 'm.room.name', 'm.room.avatar']
            AND state_key = ''
        ";
        let mut result = self
            .db
            .query(knock_state_query)
            .bind(("room_id", room_id.to_string()))
            .await?;
        let knock_state: Vec<Event> = result.take(0)?;

        Ok(KnockResult { event_id: event.event_id.clone(), knock_state })
    }

    /// Get room state IDs at specific event
    pub async fn get_room_state_ids_at_event(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<StateIdsResponse, RepositoryError> {
        // Get state event IDs at the given event
        let state_query = "
            SELECT event_id FROM event 
            WHERE room_id = $room_id 
            AND state_key IS NOT NULL
            AND origin_server_ts <= (
                SELECT origin_server_ts FROM event WHERE event_id = $event_id
            )
            ORDER BY event_type, state_key, origin_server_ts DESC
        ";
        let mut result = self
            .db
            .query(state_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let state_events: Vec<serde_json::Value> = result.take(0)?;

        let mut pdu_ids = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        for event in state_events {
            if let Some(event_id) = event.get("event_id").and_then(|v| v.as_str()) {
                let event_type = event.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
                let state_key = event.get("state_key").and_then(|v| v.as_str()).unwrap_or("");
                let key = format!("{}:{}", event_type, state_key);

                if seen_keys.insert(key) {
                    pdu_ids.push(event_id.to_string());
                }
            }
        }

        // Get auth chain IDs
        let auth_chain_query = "
            SELECT event_id FROM event 
            WHERE room_id = $room_id 
            AND event_type IN ['m.room.create', 'm.room.join_rules', 'm.room.power_levels']
            AND state_key = ''
        ";
        let mut result = self
            .db
            .query(auth_chain_query)
            .bind(("room_id", room_id.to_string()))
            .await?;
        let auth_events: Vec<serde_json::Value> = result.take(0)?;
        let auth_chain_ids: Vec<String> = auth_events
            .into_iter()
            .filter_map(|e| e.get("event_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        Ok(StateIdsResponse { pdu_ids, auth_chain_ids })
    }

    /// Get missing events for backfill
    pub async fn get_missing_events(
        &self,
        room_id: &str,
        earliest_events: &[String],
        latest_events: &[String],
        limit: u32,
    ) -> Result<Vec<Event>, RepositoryError> {
        // Simplified implementation - get events between earliest and latest
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND origin_server_ts >= (
                SELECT MIN(origin_server_ts) FROM event WHERE event_id IN $earliest_events
            )
            AND origin_server_ts <= (
                SELECT MAX(origin_server_ts) FROM event WHERE event_id IN $latest_events
            )
            ORDER BY origin_server_ts ASC
            LIMIT $limit
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("earliest_events", earliest_events.to_vec()))
            .bind(("latest_events", latest_events.to_vec()))
            .bind(("limit", limit as i64))
            .await?;
        let events: Vec<Event> = result.take(0)?;
        Ok(events)
    }

    /// Backfill events for federation
    pub async fn backfill_events(
        &self,
        room_id: &str,
        event_ids: &[String],
        limit: u32,
    ) -> Result<BackfillResponse, RepositoryError> {
        // Get events before the specified event IDs
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND origin_server_ts < (
                SELECT MIN(origin_server_ts) FROM event WHERE event_id IN $event_ids
            )
            ORDER BY origin_server_ts DESC
            LIMIT $limit
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_ids", event_ids.to_vec()))
            .bind(("limit", limit as i64))
            .await?;
        let pdus: Vec<Event> = result.take(0)?;

        Ok(BackfillResponse { pdus })
    }

    /// Validate third-party invite
    pub async fn validate_third_party_invite(
        &self,
        room_id: &str,
        invite: &ThirdPartyInvite,
    ) -> Result<bool, RepositoryError> {
        // Verify room exists
        let room_query = "SELECT room_id FROM room WHERE room_id = $room_id LIMIT 1";
        let mut result = self.db.query(room_query).bind(("room_id", room_id.to_string())).await?;
        let rooms: Vec<serde_json::Value> = result.take(0)?;
        
        if rooms.is_empty() {
            return Ok(false);
        }

        // Basic validation of third-party invite
        if invite.display_name.is_empty() || invite.public_key.is_empty() {
            return Ok(false);
        }

        // Check if the key validity URL is accessible (simplified)
        if invite.key_validity_url.is_empty() {
            return Ok(false);
        }

        // Fetch public key from identity server
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| RepositoryError::Validation {
                field: "http_client".to_string(),
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        let key_response = client
            .get(&invite.key_validity_url)
            .send()
            .await
            .map_err(|e| RepositoryError::Validation {
                field: "key_fetch".to_string(),
                message: format!("Failed to fetch identity server key from {}: {}", invite.key_validity_url, e),
            })?;

        if !key_response.status().is_success() {
            warn!(
                "Failed to fetch identity server key from {}: HTTP {}",
                invite.key_validity_url,
                key_response.status()
            );
            return Ok(false);
        }

        let key_data: serde_json::Value = key_response.json().await
            .map_err(|e| RepositoryError::Validation {
                field: "key_data".to_string(),
                message: format!("Invalid key response format: {}", e),
            })?;

        // Extract identity server's public key from response
        let public_key_b64 = key_data
            .get("public_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "public_key".to_string(),
                message: "Identity server response missing public_key field".to_string(),
            })?;

        // Extract signature from invite.public_keys
        // Look for a public_key entry with signatures
        let mut signature_b64: Option<&str> = None;
        
        for pk_entry in &invite.public_keys {
            if let Some(signatures) = pk_entry.get("signatures") {
                if let Some(sigs_obj) = signatures.as_object() {
                    // Find ed25519 signature in any server's signatures
                    for (_, key_sigs) in sigs_obj.iter() {
                        if let Some(key_sigs_obj) = key_sigs.as_object() {
                            for (key_id, sig_value) in key_sigs_obj.iter() {
                                if key_id.starts_with("ed25519:") {
                                    if let Some(sig_str) = sig_value.as_str() {
                                        signature_b64 = Some(sig_str);
                                        break;
                                    }
                                }
                            }
                            if signature_b64.is_some() {
                                break;
                            }
                        }
                    }
                }
                if signature_b64.is_some() {
                    break;
                }
            }
        }

        let signature_b64 = match signature_b64 {
            Some(sig) => sig,
            None => {
                warn!(
                    "No ed25519 signature found in third-party invite from {}",
                    invite.display_name
                );
                return Ok(false);
            }
        };

        // Create canonical JSON for verification using Matrix canonical JSON rules
        // This ensures lexicographically sorted keys as required by Matrix spec
        let invite_json = serde_json::to_value(invite)
            .map_err(|e| RepositoryError::Validation {
                field: "invite_serialize".to_string(),
                message: format!("Failed to serialize invite: {}", e),
            })?;

        // Use canonical_json_for_signing which automatically:
        // 1. Removes signatures and unsigned fields
        // 2. Sorts object keys lexicographically
        // 3. Applies Matrix canonical JSON format
        let canonical_string = canonical_json_for_signing(&invite_json)
            .map_err(|e| RepositoryError::Validation {
                field: "canonical_json".to_string(),
                message: format!("Failed to create canonical JSON: {}", e),
            })?;

        let canonical_bytes = canonical_string.as_bytes();

        // Decode public key and signature from base64
        let verify_key_bytes = general_purpose::STANDARD.decode(public_key_b64)
            .map_err(|_| RepositoryError::Validation {
                field: "public_key".to_string(),
                message: "Invalid base64 encoding in identity server public key".to_string(),
            })?;

        let signature_bytes = general_purpose::STANDARD.decode(signature_b64)
            .map_err(|_| RepositoryError::Validation {
                field: "signature".to_string(),
                message: "Invalid base64 encoding in invite signature".to_string(),
            })?;

        // Validate key and signature sizes
        if verify_key_bytes.len() != 32 {
            warn!(
                "Invalid public key size from identity server {}: {} bytes (expected 32)",
                invite.key_validity_url,
                verify_key_bytes.len()
            );
            return Ok(false);
        }

        if signature_bytes.len() != 64 {
            warn!(
                "Invalid signature size in third-party invite: {} bytes (expected 64)",
                signature_bytes.len()
            );
            return Ok(false);
        }

        // Convert to fixed-size arrays
        let key_array: [u8; 32] = match verify_key_bytes.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                return Err(RepositoryError::Validation {
                    field: "public_key".to_string(),
                    message: "Failed to convert public key bytes to array".to_string(),
                });
            }
        };

        let sig_array: [u8; 64] = match signature_bytes.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                return Err(RepositoryError::Validation {
                    field: "signature".to_string(),
                    message: "Failed to convert signature bytes to array".to_string(),
                });
            }
        };

        // Create Ed25519 verifying key and signature objects
        let verifying_key = VerifyingKey::from_bytes(&key_array)
            .map_err(|_| RepositoryError::Validation {
                field: "public_key".to_string(),
                message: "Invalid Ed25519 public key from identity server".to_string(),
            })?;

        let signature_obj = Signature::from_bytes(&sig_array);

        // Verify the signature
        match verifying_key.verify(&canonical_bytes, &signature_obj) {
            Ok(()) => {
                info!(
                    "Third-party invite signature verified successfully for {} from {}",
                    invite.display_name,
                    invite.key_validity_url
                );
                Ok(true)
            }
            Err(_) => {
                warn!(
                    "Third-party invite signature verification failed for {} from {}",
                    invite.display_name,
                    invite.key_validity_url
                );
                Ok(false)
            }
        }
    }

    /// Exchange third-party invite for membership event
    pub async fn exchange_third_party_invite(
        &self,
        room_id: &str,
        invite: &ThirdPartyInvite,
    ) -> Result<Event, RepositoryError> {
        // Validate the invite first
        if !self.validate_third_party_invite(room_id, invite).await? {
            return Err(RepositoryError::Validation {
                field: "invite".to_string(),
                message: "Invalid third-party invite".to_string(),
            });
        }

        let event_id = format!("${}", uuid::Uuid::new_v4());
        let now = chrono::Utc::now();

        let content = serde_json::json!({
            "membership": "invite",
            "displayname": invite.display_name,
            "third_party_invite": {
                "display_name": invite.display_name,
                "signed": {
                    "mxid": format!("@{}:example.com", invite.display_name.to_lowercase()),
                    "token": "token_value",
                    "signatures": {}
                }
            }
        });

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: "@system:localhost".to_string(), // System sender for third-party invites
            event_type: "m.room.member".to_string(),
            content: matryx_entity::types::EventContent::Unknown(content),
            state_key: Some(format!("@{}:example.com", invite.display_name.to_lowercase())),
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

        Ok(event)
    }

    /// Find event by content hash - used by PDU validator for duplicate detection
    pub async fn find_event_by_content_hash(
        &self,
        room_id: &str,
        content_hash: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
              AND content_hash = $content_hash 
            LIMIT 1
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("content_hash", content_hash.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "find event by content hash".to_string(),
                }
            })?;

        let existing_events: Vec<Event> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "parse event query result".to_string(),
            }
        })?;

        Ok(existing_events.into_iter().next())
    }

    /// Get current state event - used by PDU validator for state conflict detection
    pub async fn get_current_state_event(
        &self,
        room_id: &str,
        event_type: &str,
        state_key: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
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
            .bind(("room_id", room_id.to_string()))
            .bind(("event_type", event_type.to_string()))
            .bind(("state_key", state_key.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get current state event".to_string(),
                }
            })?;

        let existing_events: Vec<Event> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "parse state event query result".to_string(),
            }
        })?;

        Ok(existing_events.into_iter().next())
    }

    /// Find recent events within temporal window - used by PDU validator for spam detection
    pub async fn find_recent_events_by_sender(
        &self,
        room_id: &str,
        sender: &str,
        event_type: &str,
        start_time: i64,
        end_time: i64,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, RepositoryError> {
        let limit_clause = limit.unwrap_or(5);
        let query = format!(
            "
            SELECT * FROM event 
            WHERE room_id = $room_id 
              AND sender = $sender 
              AND event_type = $event_type 
              AND origin_server_ts >= $start_time 
              AND origin_server_ts <= $end_time
            ORDER BY origin_server_ts DESC
            LIMIT {}
            ",
            limit_clause
        );

        let mut response = self
            .db
            .query(&query)
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .bind(("event_type", event_type.to_string()))
            .bind(("start_time", start_time))
            .bind(("end_time", end_time))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "find recent events by sender".to_string(),
                }
            })?;

        let recent_events: Vec<Event> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "parse recent events query result".to_string(),
            }
        })?;

        Ok(recent_events)
    }

    /// Process typing EDU - create/update typing event
    pub async fn process_typing_edu(
        &self,
        room_id: &str,
        user_id: &str,
        server_name: &str,
        typing: bool,
    ) -> Result<(), RepositoryError> {
        if typing {
            // User started typing - create/update typing event with timeout
            let expires_at = Utc::now() + chrono::Duration::seconds(30); // 30 second timeout

            let query = "
                BEGIN;
                DELETE typing_notification WHERE room_id = $room_id AND user_id = $user_id;
                CREATE typing_notification SET
                    room_id = $room_id,
                    user_id = $user_id,
                    typing = true,
                    server_name = $server_name,
                    started_at = $started_at,
                    expires_at = $expires_at,
                    updated_at = time::now();
                COMMIT;
            ";

            self.db
                .query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("user_id", user_id.to_string()))
                .bind(("server_name", server_name.to_string()))
                .bind(("started_at", Utc::now()))
                .bind(("expires_at", expires_at))
                .await?;
        } else {
            // User stopped typing - remove typing event
            let query = "
                DELETE typing_notification
                WHERE room_id = $room_id AND user_id = $user_id
            ";

            self.db
                .query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("user_id", user_id.to_string()))
                .await?;
        }

        Ok(())
    }

    /// Process receipt EDU - store read receipt
    pub async fn process_receipt_edu(
        &self,
        room_id: &str,
        user_id: &str,
        event_id: &str,
        receipt_type: &str,
        timestamp: i64,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE receipts SET
                room_id = $room_id,
                user_id = $user_id,
                event_id = $event_id,
                receipt_type = $receipt_type,
                timestamp = $timestamp,
                created_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .bind(("receipt_type", receipt_type.to_string()))
            .bind(("timestamp", timestamp))
            .await?;

        Ok(())
    }

    /// Process presence EDU - update user presence
    pub async fn process_presence_edu(
        &self,
        user_id: &str,
        presence: &str,
        status_msg: Option<&str>,
        last_active_ago: Option<i64>,
        currently_active: bool,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPSERT presence_events:⟨$user_id⟩ CONTENT {
                user_id: $user_id,
                presence: $presence,
                status_msg: $status_msg,
                last_active_ago: $last_active_ago,
                currently_active: $currently_active,
                updated_at: time::now()
            }
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("presence", presence.to_string()))
            .bind(("status_msg", status_msg.map(|s| s.to_string())))
            .bind(("last_active_ago", last_active_ago))
            .bind(("currently_active", currently_active))
            .await?;

        Ok(())
    }

    /// Process device list EDU - update device list
    pub async fn process_device_list_edu(
        &self,
        params: ProcessDeviceListEduParams,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE device_list_updates SET
                user_id = $user_id,
                device_id = $device_id,
                stream_id = $stream_id,
                deleted = $deleted,
                prev_id = $prev_id,
                device_display_name = $device_display_name,
                keys = $keys,
                updated_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("user_id", params.user_id))
            .bind(("device_id", params.device_id))
            .bind(("stream_id", params.stream_id))
            .bind(("deleted", params.deleted))
            .bind(("prev_id", params.prev_id))
            .bind(("device_display_name", params.device_display_name))
            .bind(("keys", params.keys))
            .await?;

        Ok(())
    }

    /// Process signing key update EDU - update user signing keys
    pub async fn process_signing_key_update_edu(
        &self,
        user_id: &str,
        key_type: &str,
        keys: serde_json::Value,
        signatures: Option<serde_json::Value>,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPSERT user_signing_keys:⟨$user_id⟩ CONTENT {
                user_id: $user_id,
                key_type: $key_type,
                keys: $keys,
                signatures: $signatures,
                updated_at: time::now()
            }
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("key_type", key_type.to_string()))
            .bind(("keys", keys))
            .bind(("signatures", signatures))
            .await?;

        Ok(())
    }

    /// Process direct-to-device EDU - store direct message
    pub async fn process_direct_to_device_edu(
        &self,
        params: DirectToDeviceEduParams<'_>,
    ) -> Result<(), RepositoryError> {
        // Check for duplicates first
        let duplicate_check = "
            SELECT count() FROM direct_to_device_messages 
            WHERE message_id = $message_id AND origin = $origin
            GROUP ALL
        ";

        let mut duplicate_result = self.db
            .query(duplicate_check)
            .bind(("message_id", params.message_id.to_string()))
            .bind(("origin", params.origin.to_string()))
            .await?;

        let count: Option<i64> = duplicate_result.take(0)?;
        if count.unwrap_or(0) > 0 {
            return Ok(()); // Duplicate message, ignore
        }

        // Store the message
        let query = "
            CREATE direct_to_device_messages SET
                message_id = $message_id,
                origin = $origin,
                sender = $sender,
                message_type = $message_type,
                content = $content,
                target_user_id = $target_user_id,
                target_device_id = $target_device_id,
                created_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("message_id", params.message_id.to_string()))
            .bind(("origin", params.origin.to_string()))
            .bind(("sender", params.sender.to_string()))
            .bind(("message_type", params.message_type.to_string()))
            .bind(("content", params.content))
            .bind(("target_user_id", params.target_user_id.to_string()))
            .bind(("target_device_id", params.target_device_id.map(|s| s.to_string())))
            .await?;

        Ok(())
    }

    /// Get third-party invite event by room and token
    pub async fn get_third_party_invite_event(
        &self,
        room_id: &str,
        token: &str,
    ) -> Result<Option<serde_json::Value>, RepositoryError> {
        let query = "
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.third_party_invite' 
            AND content.token = $token
            ORDER BY origin_server_ts DESC 
            LIMIT 1
        ";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("token", token.to_string()))
            .await?;

        let event_content: Option<serde_json::Value> = response.take(0)?;
        Ok(event_content)
    }

    /// Get trusted identity servers from configuration
    pub async fn get_trusted_identity_servers(&self) -> Result<Option<Vec<String>>, RepositoryError> {
        let query = "SELECT value FROM server_config WHERE key = 'trusted_identity_servers'";

        let mut response = self.db.query(query).await?;
        let config_value: Option<serde_json::Value> = response.take(0)?;

        if let Some(value) = config_value
            && let Some(servers) = value.as_array() {
                let trusted: Vec<String> = servers
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                return Ok(Some(trusted));
            }

        Ok(None)
    }

    /// Check if server is blocked for third-party invites
    pub async fn is_server_blocked_for_third_party_invites(
        &self,
        server_name: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "SELECT blocked FROM server_blocklist WHERE server_name = $server_name AND third_party_invites = true";

        let mut response = self.db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .await?;

        let blocked: Option<bool> = response.take(0)?;
        Ok(blocked.unwrap_or(false))
    }

    /// Check server federation configuration
    pub async fn check_server_federation_config(
        &self,
        server_name: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "SELECT federation_enabled FROM server_federation_config WHERE server_name = $server_name";

        let mut response = self.db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .await?;

        let federation_enabled: Option<bool> = response.take(0)?;
        Ok(federation_enabled.unwrap_or(true)) // Default to enabled if not configured
    }

    /// Check if server is rate limited for third-party invites
    pub async fn is_server_rate_limited_for_third_party_invites(
        &self,
        server_name: &str,
    ) -> Result<bool, RepositoryError> {
        let now = chrono::Utc::now();
        let hour_ago = now - chrono::Duration::hours(1);

        // Check third-party invite rate in the last hour
        let query = "
            SELECT COUNT(*) as invite_count
            FROM third_party_invite_log
            WHERE server_name = $server_name
            AND timestamp >= $hour_ago
        ";

        let mut response = self.db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("hour_ago", hour_ago))
            .await?;

        let count: Option<i64> = response.take(0)?;
        const MAX_THIRD_PARTY_INVITES_PER_HOUR: i64 = 100;

        let is_rate_limited = count.unwrap_or(0) >= MAX_THIRD_PARTY_INVITES_PER_HOUR;

        // Log this third-party invite attempt
        if !is_rate_limited {
            let log_query = "
                INSERT INTO third_party_invite_log (server_name, timestamp)
                VALUES ($server_name, $now)
            ";

            let _ = self.db
                .query(log_query)
                .bind(("server_name", server_name.to_string()))
                .bind(("now", now))
                .await; // Ignore errors for logging
        }

        Ok(is_rate_limited)
    }

    /// Get room alias information for federation directory query
    pub async fn get_room_alias_info(
        &self,
        alias: &str,
    ) -> Result<Option<(String, Option<Vec<String>>)>, RepositoryError> {
        let query = "
            SELECT 
                room_id,
                array::distinct(
                    SELECT VALUE string::split(user_id, ':')[1]
                    FROM room_memberships
                    WHERE room_id = $parent.room_id
                    AND membership = 'join'
                ) AS servers
            FROM room_aliases
            WHERE alias = $alias
            LIMIT 1
        ";

        let mut response = self.db
            .query(query)
            .bind(("alias", alias.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct AliasResult {
            room_id: String,
            servers: Option<Vec<String>>,
        }

        let alias_result: Option<AliasResult> = response.take(0)?;
        Ok(alias_result.map(|result| (result.room_id, result.servers)))
    }
}
