use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use matryx_entity::types::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

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
        // Get server keys for origin
        let keys = self.get_server_keys(origin).await?;

        if keys.is_empty() {
            return Ok(false);
        }

        // In a real implementation, this would verify the actual cryptographic signature
        // For now, we'll check if the event has signatures from the origin server
        if let Some(signatures) = &event.signatures
            && signatures.contains_key(origin) {
                return Ok(true);
            }

        Ok(false)
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
                    if let Some(pattern_str) = pattern.as_str() {
                        if self.matches_server_pattern(server_name, pattern_str) {
                            return Ok(false);
                        }
                    }
                }
            }

            // Check allow list
            if let Some(allow_list) = content.get("allow").and_then(|v| v.as_array()) {
                for pattern in allow_list {
                    if let Some(pattern_str) = pattern.as_str() {
                        if self.matches_server_pattern(server_name, pattern_str) {
                            return Ok(true);
                        }
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

        // In a real implementation, this would:
        // 1. Fetch the public key from the key validity URL
        // 2. Verify the signature on the invite
        // 3. Check the invite hasn't expired

        Ok(true)
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
            SELECT * FROM events 
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
            SELECT * FROM events 
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
}
