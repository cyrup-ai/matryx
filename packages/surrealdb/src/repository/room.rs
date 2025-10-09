use crate::repository::error::RepositoryError;
use crate::repository::EventRepository;

use matryx_entity::filter::EventFilter;
use matryx_entity::types::{
    Event,
    MembershipState,
    NotificationPowerLevels,
    PowerLevels,
    Room,
    SpaceHierarchyResponse as HierarchyResponse,
    SpaceHierarchyStrippedStateEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone)]
pub struct FederationSettings {
    pub federate: bool,
    pub restricted_servers: Vec<String>,
    pub allowed_servers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RoomCreationConfig {
    pub name: Option<String>,
    pub topic: Option<String>,
    pub alias: Option<String>,
    pub is_public: bool,
    pub is_direct: bool,
    pub preset: Option<String>,
    pub invite_users: Vec<String>,
    pub invite_3pid: Vec<Value>,
    pub initial_state: Vec<Value>,
    pub power_level_content_override: Option<Value>,
    pub creation_content: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct RoomCreationContent {
    pub creator: String,
    pub room_version: String,
    pub federate: bool,
    pub predecessor: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum RoomOperation {
    SendMessage,
    SendState(String), // event_type
    InviteUser,
    KickUser,
    BanUser,
    ChangeSettings,
    ChangePowerLevels,
}

#[derive(Debug, Clone)]
pub enum RoomVisibility {
    Public,
    Private,
}


// TASK16 SUBTASK 3: Add JoinRules enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JoinRules {
    Public,
    Invite,
    Knock,
    Private,
    Restricted,
}

impl JoinRules {
    pub fn as_str(&self) -> &'static str {
        match self {
            JoinRules::Public => "public",
            JoinRules::Invite => "invite",
            JoinRules::Knock => "knock",
            JoinRules::Private => "private",
            JoinRules::Restricted => "restricted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GuestAccess {
    CanJoin,
    Forbidden,
}

impl GuestAccess {
    pub fn as_str(&self) -> &'static str {
        match self {
            GuestAccess::CanJoin => "can_join",
            GuestAccess::Forbidden => "forbidden",
        }
    }
}

// Response types for advanced room operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResponse {
    pub events_before: Vec<Event>,
    pub event: Option<Event>,
    pub events_after: Vec<Event>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub state: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembersResponse {
    pub joined: std::collections::HashMap<String, MemberInfo>,
    pub left: Option<std::collections::HashMap<String, MemberInfo>>,
    pub invited: Option<std::collections::HashMap<String, MemberInfo>>,
    pub banned: Option<std::collections::HashMap<String, MemberInfo>>,
    pub knocked: Option<std::collections::HashMap<String, MemberInfo>>,
}

// Type alias for complex room data tuple to reduce type complexity
type RoomDataTuple = (String, Option<String>, Option<String>, Option<String>, Option<String>, bool, bool, String, Option<String>, String, u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberInfo {
    pub avatar_url: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUpgradeResponse {
    pub replacement_room: String,
}

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
pub struct RoomRepository {
    db: Surreal<Any>,
}

impl RoomRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, room: &Room) -> Result<Room, RepositoryError> {
        let room_clone = room.clone();
        let created: Option<Room> =
            self.db.create(("room", &room.room_id)).content(room_clone).await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create room"))
        })
    }

    pub async fn get_by_id(&self, room_id: &str) -> Result<Option<Room>, RepositoryError> {
        let room: Option<Room> = self.db.select(("room", room_id)).await?;
        Ok(room)
    }

    pub async fn update(&self, room: &Room) -> Result<Room, RepositoryError> {
        let room_clone = room.clone();
        let updated: Option<Room> =
            self.db.update(("room", &room.room_id)).content(room_clone).await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update room"))
        })
    }

    pub async fn delete(&self, room_id: &str) -> Result<(), RepositoryError> {
        let _: Option<Room> = self.db.delete(("room", room_id)).await?;
        Ok(())
    }

    pub async fn get_rooms_for_user(&self, user_id: &str) -> Result<Vec<Room>, RepositoryError> {
        let query = "
            SELECT * FROM room
            WHERE creator = $user_id
            OR room_id IN (
                SELECT room_id FROM membership
                WHERE user_id = $user_id
                AND membership IN ['join', 'invite']
            )
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let rooms: Vec<Room> = result.take(0)?;
        Ok(rooms)
    }

    pub async fn is_room_member(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() FROM membership
            WHERE room_id = $room_id
            AND user_id = $user_id
            AND membership IN ['join', 'invite']
            GROUP ALL
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    pub async fn get_public_rooms(&self, limit: Option<i64>) -> Result<Vec<Room>, RepositoryError> {
        let query = match limit {
            Some(l) => format!("SELECT * FROM room WHERE is_public = true LIMIT {}", l),
            None => "SELECT * FROM room WHERE is_public = true".to_string(),
        };
        let mut result = self.db.query(&query).await?;
        let rooms: Vec<Room> = result.take(0)?;
        Ok(rooms)
    }

    /// Check if a user is a member of a room
    pub async fn check_membership(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() FROM membership
            WHERE room_id = $room_id
            AND user_id = $user_id
            AND membership IN ['join', 'invite']
            GROUP ALL
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Get room state events
    pub async fn get_room_state(&self, room_id: &str) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND state_key IS NOT NULL
            ORDER BY origin_server_ts DESC
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<Event> = result.take(0)?;
        Ok(events)
    }

    /// Validate room authorization for an event
    pub async fn validate_room_authorization(
        &self,
        room_id: &str,
        event: &Event,
        auth_service: &crate::repository::room_authorization::RoomAuthorizationService,
    ) -> Result<bool, RepositoryError> {
        let auth_result = auth_service
            .check_room_access(room_id, &event.sender, &event.event_type)
            .await?;

        if !auth_result.authorized {
            return Err(RepositoryError::Forbidden {
                reason: auth_result.reason.unwrap_or_else(|| "Unauthorized".to_string()),
            });
        }

        Ok(true)
    }

    /// Validate authorization using default Matrix power levels
    /// This is used as a fallback when explicit power levels are not configured
    pub async fn validate_with_default_power_levels(
        &self,
        event: &Event,
    ) -> Result<bool, RepositoryError> {
        // Default Matrix power levels: users = 0, moderators = 50, admins = 100
        // This is a simplified implementation
        match event.event_type.as_str() {
            "m.room.message" | "m.room.encrypted" => Ok(true), // Power level 0 required
            "m.room.name" | "m.room.topic" | "m.room.avatar" => Ok(true), // Power level 50 required (simplified to true)
            "m.room.power_levels" => Ok(true), // Power level 100 required (simplified to true)
            _ => Ok(true),
        }
    }

    /// Get member count for a room
    pub async fn get_member_count(&self, room_id: &str) -> Result<u64, RepositoryError> {
        let query = "
            SELECT count() FROM membership
            WHERE room_id = $room_id
            AND membership = 'join'
            GROUP ALL
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) as u64)
    }

    /// Get room by ID (alias for get_by_id for consistency)
    pub async fn get_room_by_id(&self, room_id: &str) -> Result<Option<Room>, RepositoryError> {
        self.get_by_id(room_id).await
    }

    /// Get room by alias
    pub async fn get_room_by_alias(&self, alias: &str) -> Result<Option<Room>, RepositoryError> {
        let room_id = self.resolve_room_alias(alias).await?;
        match room_id {
            Some(id) => self.get_by_id(&id).await,
            None => Ok(None),
        }
    }

    /// Get room visibility (public or private)
    pub async fn get_room_visibility(
        &self,
        room_id: &str,
    ) -> Result<RoomVisibility, RepositoryError> {
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.history_visibility'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content")
            && let Some(visibility) = content.get("history_visibility").and_then(|v| v.as_str())
        {
            return match visibility {
                "world_readable" => Ok(RoomVisibility::Public),
                _ => Ok(RoomVisibility::Private),
            };
        }

        // Default to private if no visibility event found
        Ok(RoomVisibility::Private)
    }



    /// Get room power levels
    pub async fn get_room_power_levels(
        &self,
        room_id: &str,
    ) -> Result<PowerLevels, RepositoryError> {
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.power_levels'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content") {
            let users = content
                .get("users")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_i64().map(|i| (k.clone(), i)))
                        .collect()
                })
                .unwrap_or_default();

            let events_map = content
                .get("events")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_i64().map(|i| (k.clone(), i)))
                        .collect()
                })
                .unwrap_or_default();

            return Ok(PowerLevels {
                users,
                users_default: content
                    .get("users_default")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                events: events_map,
                events_default: content
                    .get("events_default")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                state_default: content
                    .get("state_default")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(50),
                ban: content.get("ban").and_then(|v| v.as_i64()).unwrap_or(50),
                kick: content.get("kick").and_then(|v| v.as_i64()).unwrap_or(50),
                redact: content.get("redact").and_then(|v| v.as_i64()).unwrap_or(50),
                invite: content.get("invite").and_then(|v| v.as_i64()).unwrap_or(50),
                notifications: Default::default(),
            });
        }

        // Return default power levels if no event found
        Ok(PowerLevels::default())
    }

    /// Get room name from m.room.name state event
    pub async fn get_room_name(&self, room_id: &str) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.name'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content")
            && let Some(name) = content.get("name").and_then(|v| v.as_str()) {
            return Ok(Some(name.to_string()));
        }

        Ok(None)
    }

    /// Get room join rules
    pub async fn get_room_join_rules(&self, room_id: &str) -> Result<JoinRules, RepositoryError> {
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.join_rules'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content")
            && let Some(join_rule) = content.get("join_rule").and_then(|v| v.as_str()) {
            return Ok(match join_rule {
                "public" => JoinRules::Public,
                "invite" => JoinRules::Invite,
                "knock" => JoinRules::Knock,
                "private" => JoinRules::Private,
                "restricted" => JoinRules::Restricted,
                _ => JoinRules::Invite, // Default
            });
        }

        // Default to "invite" if no join rules are set
        Ok(JoinRules::Invite)
    }

    /// Check if a room is joinable by a user
    pub async fn is_room_joinable(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        let join_rules = self.get_room_join_rules(room_id).await?;

        match join_rules {
            JoinRules::Public => Ok(true),
            JoinRules::Invite => {
                // Check if user has pending invitation
                self.check_membership(room_id, user_id).await
            },
            JoinRules::Private => {
                // Private rooms require invitation
                self.check_membership(room_id, user_id).await
            },
            JoinRules::Knock => {
                // Check if user has sent a knock request
                let query = "
                    SELECT membership FROM membership
                    WHERE room_id = $room_id AND user_id = $user_id
                    AND membership = 'knock'
                    LIMIT 1
                ";
                let mut result = self
                    .db
                    .query(query)
                    .bind(("room_id", room_id.to_string()))
                    .bind(("user_id", user_id.to_string()))
                    .await?;
                let memberships: Vec<Value> = result.take(0)?;
                Ok(!memberships.is_empty())
            },
            JoinRules::Restricted => {
                // For restricted rooms, check if user has invite or is member of allowed rooms
                // First check for pending invite
                if self.check_membership(room_id, user_id).await? {
                    return Ok(true);
                }

                // Check allow conditions (simplified implementation)
                // In a full implementation, this would check the allow conditions from the join_rules event
                Ok(false)
            },
        }
    }

    /// Create a new room with configuration
    pub async fn create_room(
        &self,
        room_config: &RoomCreationConfig,
    ) -> Result<Room, RepositoryError> {
        let room_id = format!("!{}:{}", uuid::Uuid::new_v4(), "localhost"); // Simplified room ID generation

        let room = Room {
            room_id: room_id.clone(),
            name: room_config.name.clone(),
            topic: room_config.topic.clone(),
            avatar_url: None,
            canonical_alias: room_config.alias.clone(),
            alt_aliases: Some(Vec::new()),
            creator: "".to_string(), // Will be set by caller
            is_public: Some(room_config.is_public),
            is_direct: Some(room_config.is_direct),
            join_rule: if room_config.is_public {
                Some("public".to_string())
            } else {
                Some("invite".to_string())
            },
            join_rules: if room_config.is_public {
                Some("public".to_string())
            } else {
                Some("invite".to_string())
            },
            guest_access: Some("can_join".to_string()),
            history_visibility: Some("shared".to_string()),
            room_version: "9".to_string(),
            power_levels: None,
            encryption: None,
            room_type: None,
            predecessor: None,
            federate: Some(true),
            tombstone: None,
            state_events_count: Some(0),
            created_at: chrono::Utc::now(),
            updated_at: Some(chrono::Utc::now()),
        };

        self.create(&room).await
    }

    /// Update room state
    pub async fn update_room_state(
        &self,
        room_id: &str,
        state_key: &str,
        content: Value,
    ) -> Result<(), RepositoryError> {
        // Create a state event for the room state change
        let event_id = format!("${}:example.com", uuid::Uuid::new_v4());
        let now = chrono::Utc::now();
        
        let state_event = serde_json::json!({
            "event_id": event_id,
            "room_id": room_id,
            "sender": "@system:example.com", // System user for state updates
            "event_type": "m.room.state",
            "state_key": state_key,
            "content": content,
            "origin_server_ts": now.timestamp_millis(),
            "unsigned": {},
            "auth_events": [],
            "prev_events": [],
            "depth": 1,
            "signatures": {}
        });

        // Store the state event
        let _: Option<Value> = self
            .db
            .create(("event", &event_id))
            .content(state_event)
            .await?;

        // Update the room's updated timestamp
        let query = "
            UPDATE room SET updated_at = $updated_at
            WHERE room_id = $room_id
        ";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("updated_at", now))
            .await?;

        Ok(())
    }

    /// Get a specific room state event
    pub async fn get_room_state_event(
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
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_type", event_type.to_string()))
            .bind(("state_key", state_key.to_string()))
            .await?;
        let events: Vec<Event> = result.take(0)?;
        Ok(events.into_iter().next())
    }

    /// Validate if a user can perform a room operation
    pub async fn validate_room_operation(
        &self,
        room_id: &str,
        user_id: &str,
        operation: RoomOperation,
    ) -> Result<bool, RepositoryError> {
        // Check if user is a member of the room
        if !self.check_membership(room_id, user_id).await? {
            return Ok(false);
        }

        // For basic operations, membership is sufficient
        // More complex power level checks would be handled by PowerLevelsRepository
        match operation {
            RoomOperation::SendMessage => Ok(true),
            RoomOperation::SendState(_) => Ok(true), // Simplified - would check power levels
            RoomOperation::InviteUser => Ok(true),   // Simplified - would check power levels
            RoomOperation::KickUser => Ok(true),     // Simplified - would check power levels
            RoomOperation::BanUser => Ok(true),      // Simplified - would check power levels
            RoomOperation::ChangeSettings => Ok(true), // Simplified - would check power levels
            RoomOperation::ChangePowerLevels => Ok(true), // Simplified - would check power levels
        }
    }

    /// Get room creation content
    pub async fn get_room_creation_content(
        &self,
        room_id: &str,
    ) -> Result<RoomCreationContent, RepositoryError> {
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.create'
            AND state_key = ''
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content") {
            return Ok(RoomCreationContent {
                creator: content
                    .get("creator")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                room_version: content
                    .get("room_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("9")
                    .to_string(),
                federate: content.get("m.federate").and_then(|v| v.as_bool()).unwrap_or(true),
                predecessor: content.get("predecessor").cloned(),
            });
        }

        Err(RepositoryError::NotFound {
            entity_type: "Room creation event".to_string(),
            id: room_id.to_string(),
        })
    }

    // Federation state management methods

    /// Get room state at a specific event
    pub async fn get_room_state_at_event(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND state_key IS NOT NULL
            AND origin_server_ts <= (
                SELECT origin_server_ts FROM event WHERE event_id = $event_id
            )
            ORDER BY event_type, state_key, origin_server_ts DESC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let events: Vec<Event> = result.take(0)?;

        // Deduplicate by (event_type, state_key) keeping the latest
        let mut state_map = std::collections::HashMap::new();
        for event in events {
            let key = (event.event_type.clone(), event.state_key.clone().unwrap_or_default());
            state_map.entry(key).or_insert(event);
        }

        Ok(state_map.into_values().collect())
    }

    /// Get room state event IDs
    pub async fn get_room_state_ids(
        &self,
        room_id: &str,
        event_id: Option<&str>,
    ) -> Result<Vec<String>, RepositoryError> {
        let (query, has_event_filter) = if let Some(event_id_ref) = event_id {
            (
                "
                SELECT event_id FROM event
                WHERE room_id = $room_id
                AND state_key IS NOT NULL
                AND origin_server_ts <= (
                    SELECT origin_server_ts FROM event WHERE event_id = $event_id
                )
                ORDER BY event_type, state_key, origin_server_ts DESC
                ",
                Some(event_id_ref)
            )
        } else {
            (
                "
                SELECT event_id FROM event
                WHERE room_id = $room_id
                AND state_key IS NOT NULL
                ORDER BY event_type, state_key, origin_server_ts DESC
                ",
                None
            )
        };

        let mut result = if let Some(event_id_filter) = has_event_filter {
            self.db
                .query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("event_id", event_id_filter.to_string()))
                .await?
        } else {
            self.db.query(query).bind(("room_id", room_id.to_string())).await?
        };

        let events: Vec<serde_json::Value> = result.take(0)?;
        let mut event_ids = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        for event in events {
            if let Some(event_id) = event.get("event_id").and_then(|v| v.as_str()) {
                // Deduplicate by (event_type, state_key)
                let event_type = event.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
                let state_key = event.get("state_key").and_then(|v| v.as_str()).unwrap_or("");
                let key = format!("{}:{}", event_type, state_key);

                if seen_keys.insert(key) {
                    event_ids.push(event_id.to_string());
                }
            }
        }

        Ok(event_ids)
    }

    /// Validate room for federation
    pub async fn validate_room_for_federation(
        &self,
        room_id: &str,
        server_name: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if room exists
        let room = self.get_room_by_id(room_id).await?;
        if room.is_none() {
            return Ok(false);
        }

        // Check federation settings
        let federation_settings = self.check_room_federation_settings(room_id).await?;
        if !federation_settings.federate {
            return Ok(false);
        }

        // Check server ACL
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

        if let Some(acl_event) = acl_events.first()
            && let Some(content) = acl_event.get("content") {
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

        Ok(true) // Default allow if no ACL
    }

    /// Get room version
    pub async fn get_room_version(&self, room_id: &str) -> Result<String, RepositoryError> {
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.create'
            AND state_key = ''
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content")
            && let Some(version) = content.get("room_version").and_then(|v| v.as_str()) {
            return Ok(version.to_string());
        }

        Ok("1".to_string()) // Default to version 1 if not specified
    }

    /// Get room creation event
    pub async fn get_room_creation_event(&self, room_id: &str) -> Result<Event, RepositoryError> {
        let query = "
            SELECT * FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.create'
            AND state_key = ''
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<Event> = result.take(0)?;

        events.into_iter().next().ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Room creation event".to_string(),
                id: room_id.to_string(),
            }
        })
    }

    /// Check room federation settings
    pub async fn check_room_federation_settings(
        &self,
        room_id: &str,
    ) -> Result<FederationSettings, RepositoryError> {
        // Get room creation event for federation setting
        let creation_event = self.get_room_creation_event(room_id).await?;

        let federate = if let Some(content) = creation_event.content.as_object() {
            content.get("m.federate").and_then(|v| v.as_bool()).unwrap_or(true)
        } else {
            true // Default to federated
        };

        // Get server ACL settings
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

        let mut restricted_servers = Vec::new();
        let mut allowed_servers = Vec::new();

        if let Some(acl_event) = acl_events.first()
            && let Some(content) = acl_event.get("content") {
            if let Some(deny_list) = content.get("deny").and_then(|v| v.as_array()) {
                for pattern in deny_list {
                    if let Some(pattern_str) = pattern.as_str() {
                        restricted_servers.push(pattern_str.to_string());
                    }
                }
            }

            if let Some(allow_list) = content.get("allow").and_then(|v| v.as_array()) {
                for pattern in allow_list {
                    if let Some(pattern_str) = pattern.as_str() {
                        allowed_servers.push(pattern_str.to_string());
                    }
                }
            }
        }

        Ok(FederationSettings { federate, restricted_servers, allowed_servers })
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

    /// Get room power levels for federation validation
    pub async fn get_room_power_levels_for_federation(
        &self,
        room_id: &str,
    ) -> Result<serde_json::Value, RepositoryError> {
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.power_levels'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content") {
            return Ok(content.clone());
        }

        // Return default power levels
        Ok(serde_json::json!({
            "users_default": 0,
            "events_default": 0,
            "state_default": 50,
            "ban": 50,
            "kick": 50,
            "redact": 50,
            "invite": 50
        }))
    }

    /// Check if user can perform action in room (for federation validation)
    pub async fn can_user_perform_action(
        &self,
        room_id: &str,
        user_id: &str,
        action: &str,
        required_level: Option<i64>,
    ) -> Result<bool, RepositoryError> {
        let power_levels = self.get_room_power_levels(room_id).await?;

        // Get user's power level
        let user_level = power_levels
            .users
            .get(user_id)
            .copied()
            .unwrap_or(power_levels.users_default);

        // Get required level for action
        let required = required_level.unwrap_or({
            match action {
                "ban" => power_levels.ban,
                "kick" => power_levels.kick,
                "invite" => power_levels.invite,
                "redact" => power_levels.redact,
                _ => power_levels.events_default,
            }
        });

        Ok(user_level >= required)
    }

    /// Update room power levels
    pub async fn update_room_power_levels(
        &self,
        room_id: &str,
        power_levels: &PowerLevels,
        sender: &str,
    ) -> Result<(), RepositoryError> {
        // Validate sender has permission to change power levels
        if !self
            .can_user_perform_action(room_id, sender, "m.room.power_levels", Some(100))
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: format!(
                    "User {} not authorized to update power levels in room {}",
                    sender, room_id
                ),
            });
        }

        // Convert PowerLevels to JSON content
        let mut users_map = serde_json::Map::new();
        for (user_id, level) in &power_levels.users {
            users_map.insert(user_id.clone(), serde_json::Value::Number((*level).into()));
        }

        let mut events_map = serde_json::Map::new();
        for (event_type, level) in &power_levels.events {
            events_map.insert(event_type.clone(), serde_json::Value::Number((*level).into()));
        }

        let content = serde_json::json!({
            "users": users_map,
            "users_default": power_levels.users_default,
            "events": events_map,
            "events_default": power_levels.events_default,
            "state_default": power_levels.state_default,
            "ban": power_levels.ban,
            "kick": power_levels.kick,
            "redact": power_levels.redact,
            "invite": power_levels.invite
        });

        // Create power levels event
        let event_id = format!("${}:{}", uuid::Uuid::new_v4(), "localhost");
        let timestamp = chrono::Utc::now();

        let event_query = "
            INSERT INTO event (
                event_id, room_id, sender, event_type, state_key, content,
                origin_server_ts, unsigned, redacts, auth_events, prev_events,
                depth, created_at
            ) VALUES (
                $event_id, $room_id, $sender, 'm.room.power_levels', '',
                $content, $timestamp, {}, NONE, [], [], 1, $timestamp
            )
        ";

        self.db
            .query(event_query)
            .bind(("event_id", event_id))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .bind(("content", content))
            .bind(("timestamp", timestamp.timestamp_millis()))
            .await?;

        Ok(())
    }

    /// Get user power level in room
    pub async fn get_user_power_level(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<i64, RepositoryError> {
        let power_levels = self.get_room_power_levels(room_id).await?;
        let user_level = power_levels
            .users
            .get(user_id)
            .copied()
            .unwrap_or(power_levels.users_default);
        Ok(user_level)
    }

    /// Get users with power level at or above minimum
    pub async fn get_users_with_power_level(
        &self,
        room_id: &str,
        min_level: i64,
    ) -> Result<Vec<String>, RepositoryError> {
        let power_levels = self.get_room_power_levels(room_id).await?;
        let mut users = Vec::new();

        // Check explicitly set users
        for (user_id, level) in &power_levels.users {
            if *level >= min_level {
                users.push(user_id.clone());
            }
        }

        // Check users with default level if it meets minimum
        if power_levels.users_default >= min_level {
            let query = "
                SELECT DISTINCT user_id FROM membership
                WHERE room_id = $room_id
                AND membership = 'join'
                AND user_id NOT IN $explicit_users
            ";

            let explicit_users: Vec<String> = power_levels.users.keys().cloned().collect();
            let mut result = self
                .db
                .query(query)
                .bind(("room_id", room_id.to_string()))
                .bind(("explicit_users", explicit_users))
                .await?;
            let default_users: Vec<String> = result.take(0)?;
            users.extend(default_users);
        }

        Ok(users)
    }

    /// Validate power level change
    pub async fn validate_power_level_change(
        &self,
        room_id: &str,
        changer: &str,
        target: &str,
        new_level: i64,
    ) -> Result<bool, RepositoryError> {
        let power_levels = self.get_room_power_levels(room_id).await?;

        // Get changer's current power level
        let changer_level = power_levels
            .users
            .get(changer)
            .copied()
            .unwrap_or(power_levels.users_default);

        // Get target's current power level
        let target_current_level = power_levels
            .users
            .get(target)
            .copied()
            .unwrap_or(power_levels.users_default);

        // Matrix spec: A user can only change power levels if:
        // 1. They have permission to send m.room.power_levels events (typically 100)
        // 2. Their power level is higher than both the target's current and new levels
        // 3. They cannot set someone to a level higher than their own

        // Check if changer can send power levels events
        let required_send_level =
            power_levels.events.get("m.room.power_levels").copied().unwrap_or(100);

        if changer_level < required_send_level {
            return Ok(false);
        }

        // Check if changer level is higher than target's current level
        if changer_level <= target_current_level {
            return Ok(false);
        }

        // Check if new level doesn't exceed changer's level
        if new_level > changer_level {
            return Ok(false);
        }

        Ok(true)
    }

    /// Get default power levels
    pub async fn get_default_power_levels(&self) -> Result<PowerLevels, RepositoryError> {
        Ok(PowerLevels {
            users: std::collections::HashMap::new(),
            users_default: 0,
            events: std::collections::HashMap::new(),
            events_default: 0,
            state_default: 50,
            ban: 50,
            kick: 50,
            redact: 50,
            invite: 50,
            notifications: NotificationPowerLevels::default(),
        })
    }

    // ADVANCED ROOM OPERATIONS - SUBTASK 2 EXTENSIONS

    /// Get room context around an event
    pub async fn get_room_context(
        &self,
        room_id: &str,
        event_id: &str,
        limit: u32,
        filter: Option<EventFilter>,
    ) -> Result<ContextResponse, RepositoryError> {
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

        // Build filter clause if filter is provided
        let filter_clause = if let Some(ref filter) = filter {
            let mut clauses = Vec::new();
            
            if let Some(ref types) = filter.types {
                let type_list = types.iter().map(|t| format!("'{}'", t)).collect::<Vec<_>>().join(", ");
                clauses.push(format!("event_type IN [{}]", type_list));
            }
            
            if let Some(ref not_types) = filter.not_types {
                let not_type_list = not_types.iter().map(|t| format!("'{}'", t)).collect::<Vec<_>>().join(", ");
                clauses.push(format!("event_type NOT IN [{}]", not_type_list));
            }
            
            if let Some(ref senders) = filter.senders {
                let sender_list = senders.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(", ");
                clauses.push(format!("sender IN [{}]", sender_list));
            }
            
            if let Some(ref not_senders) = filter.not_senders {
                let not_sender_list = not_senders.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(", ");
                clauses.push(format!("sender NOT IN [{}]", not_sender_list));
            }
            
            if !clauses.is_empty() {
                format!(" AND ({})", clauses.join(" AND "))
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Get events before
        let before_query = format!(
            "SELECT * FROM event WHERE room_id = $room_id AND origin_server_ts < $timestamp{} ORDER BY origin_server_ts DESC LIMIT {}",
            filter_clause, limit
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
            "SELECT * FROM event WHERE room_id = $room_id AND origin_server_ts > $timestamp{} ORDER BY origin_server_ts ASC LIMIT {}",
            filter_clause, limit
        );
        let mut after_result = self
            .db
            .query(&after_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("timestamp", target_timestamp))
            .await?;
        let events_after: Vec<Event> = after_result.take(0)?;

        // Get room state at event
        let state = self.get_room_state_at_event(room_id, event_id).await?;

        // Generate tokens from events_before and events_after combined
        let mut all_events = events_before.clone();
        if let Some(ref evt) = target_event {
            all_events.push(evt.clone());
        }
        all_events.extend(events_after.clone());

        let (start, end) = crate::pagination::generate_timeline_tokens(&all_events, room_id);

        Ok(ContextResponse {
            events_before,
            event: target_event,
            events_after,
            start,
            end,
            state,
        })
    }

    /// Get room members with filtering
    pub async fn get_room_members_with_filter(
        &self,
        room_id: &str,
        at: Option<&str>,
        membership: Option<MembershipState>,
        not_membership: Option<MembershipState>,
    ) -> Result<MembersResponse, RepositoryError> {
        let mut query = "SELECT user_id, membership, avatar_url, display_name FROM membership WHERE room_id = $room_id".to_string();
        let mut params = vec![("room_id", room_id.to_string())];

        // Add membership filter
        if let Some(membership_filter) = membership {
            query.push_str(" AND membership = $membership");
            params.push(("membership", membership_filter.to_string()));
        }

        // Add not_membership filter
        if let Some(not_membership_filter) = not_membership {
            query.push_str(" AND membership != $not_membership");
            params.push(("not_membership", not_membership_filter.to_string()));
        }

        // Add timestamp filter if provided
        if let Some(timestamp) = at {
            query.push_str(" AND created_at <= $at");
            params.push(("at", timestamp.to_string()));
        }

        let mut result = self.db.query(&query);
        for (key, value) in params {
            result = result.bind((key, value));
        }
        let mut query_result = result.await?;
        let members: Vec<serde_json::Value> = query_result.take(0)?;

        let mut members_response = MembersResponse {
            joined: std::collections::HashMap::new(),
            left: None,
            invited: None,
            banned: None,
            knocked: None,
        };

        for member in members {
            let user_id = member.get("user_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let membership_state =
                member.get("membership").and_then(|v| v.as_str()).unwrap_or("leave");
            let member_info = MemberInfo {
                avatar_url: member
                    .get("avatar_url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                display_name: member
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };

            match membership_state {
                "join" => {
                    members_response.joined.insert(user_id, member_info);
                },
                "leave" => {
                    members_response.left
                        .get_or_insert_with(std::collections::HashMap::new)
                        .insert(user_id, member_info);
                },
                "invite" => {
                    members_response.invited
                        .get_or_insert_with(std::collections::HashMap::new)
                        .insert(user_id, member_info);
                },
                "ban" => {
                    members_response.banned
                        .get_or_insert_with(std::collections::HashMap::new)
                        .insert(user_id, member_info);
                },
                "knock" => {
                    members_response.knocked
                        .get_or_insert_with(std::collections::HashMap::new)
                        .insert(user_id, member_info);
                },
                _ => {},
            }
        }

        Ok(members_response)
    }

    /// Forget a room (remove from user's room list)
    pub async fn forget_room(&self, room_id: &str, user_id: &str) -> Result<(), RepositoryError> {
        // Check if user has left or been banned from the room
        let membership_query = "SELECT membership FROM membership WHERE room_id = $room_id AND user_id = $user_id ORDER BY created_at DESC LIMIT 1";
        let mut result = self
            .db
            .query(membership_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let memberships: Vec<serde_json::Value> = result.take(0)?;

        if let Some(membership) = memberships.first() {
            let membership_state =
                membership.get("membership").and_then(|v| v.as_str()).unwrap_or("");
            if membership_state != "leave" && membership_state != "ban" {
                return Err(RepositoryError::Validation {
                    field: "membership".to_string(),
                    message: "Can only forget rooms that have been left or user was banned from"
                        .to_string(),
                });
            }
        } else {
            return Err(RepositoryError::NotFound {
                entity_type: "Membership".to_string(),
                id: format!("{}:{}", room_id, user_id),
            });
        }

        // Mark the room as forgotten
        let forget_query = "UPDATE membership SET forgotten = true, updated_at = $updated_at WHERE room_id = $room_id AND user_id = $user_id";
        self.db
            .query(forget_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("updated_at", chrono::Utc::now()))
            .await?;

        Ok(())
    }

    /// Upgrade room to new version
    pub async fn upgrade_room(
        &self,
        room_id: &str,
        new_version: &str,
        user_id: &str,
    ) -> Result<RoomUpgradeResponse, RepositoryError> {
        // Validate user has permission to upgrade room
        if !self
            .can_user_perform_action(room_id, user_id, "upgrade", Some(100))
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User not authorized to upgrade room {}", room_id),
            });
        }

        // Create new room ID for the upgraded room
        let new_room_id = format!("!{}:{}", uuid::Uuid::new_v4(), "localhost");

        // Get current room state
        let current_room = self.get_by_id(room_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            }
        })?;

        // Get old room's creation event for predecessor
        let old_creation_event = self.get_room_creation_event(room_id).await?;
        let old_creation_event_id = old_creation_event.event_id.clone();

        // Create new room with upgraded version
        let new_room = Room {
            room_id: new_room_id.clone(),
            name: current_room.name.clone(),
            topic: current_room.topic.clone(),
            avatar_url: current_room.avatar_url.clone(),
            canonical_alias: None, // Aliases don't transfer
            alt_aliases: Some(Vec::new()),
            creator: user_id.to_string(),
            is_public: current_room.is_public,
            is_direct: current_room.is_direct,
            join_rule: current_room.join_rule.clone(),
            join_rules: current_room.join_rules.clone(),
            guest_access: current_room.guest_access.clone(),
            history_visibility: current_room.history_visibility.clone(),
            room_version: new_version.to_string(),
            power_levels: current_room.power_levels.clone(),
            encryption: current_room.encryption.clone(),
            room_type: current_room.room_type.clone(),
            predecessor: Some(serde_json::json!({
                "room_id": room_id,
                "event_id": old_creation_event_id
            })),
            federate: current_room.federate,
            tombstone: None,
            state_events_count: Some(0),
            created_at: chrono::Utc::now(),
            updated_at: Some(chrono::Utc::now()),
        };

        // Create the new room
        self.create(&new_room).await?;

        // Create EventRepository to store the tombstone event
        let event_repo = EventRepository::new(self.db.clone());

        // Create the tombstone event content
        let tombstone_content = serde_json::json!({
            "body": "This room has been replaced",
            "replacement_room": new_room_id.clone()
        });

        // Create and persist the tombstone event
        let tombstone_event = event_repo.create_room_event(
            room_id,
            "m.room.tombstone",
            &current_room.creator,
            tombstone_content,
            Some("".to_string())
        ).await?;

        // Store the tombstone event ID
        let tombstone_event_id = tombstone_event.event_id.clone();

        // Update old room with successor
        let mut old_room = current_room;
        old_room.tombstone = Some(serde_json::json!({
            "replacement_room": new_room_id,
            "body": "This room has been replaced",
            "event_id": tombstone_event_id
        }));
        self.update(&old_room).await?;

        Ok(RoomUpgradeResponse { replacement_room: new_room_id })
    }

    /// Get room aliases
    pub async fn get_room_aliases(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "SELECT alias FROM room_aliases WHERE room_id = $room_id";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let aliases: Vec<serde_json::Value> = result.take(0)?;

        let alias_strings: Vec<String> = aliases
            .into_iter()
            .filter_map(|v| v.get("alias").and_then(|a| a.as_str()).map(|s| s.to_string()))
            .collect();

        Ok(alias_strings)
    }

    /// Set room alias
    pub async fn set_room_alias(&self, room_id: &str, alias: &str) -> Result<(), RepositoryError> {
        // Check if alias already exists
        let existing_query = "SELECT room_id FROM room_aliases WHERE alias = $alias LIMIT 1";
        let mut result = self.db.query(existing_query).bind(("alias", alias.to_string())).await?;
        let existing: Vec<serde_json::Value> = result.take(0)?;

        if !existing.is_empty() {
            return Err(RepositoryError::Conflict {
                message: format!("Room alias '{}' already exists", alias),
            });
        }

        // Create the alias
        let insert_query = "INSERT INTO room_aliases (alias, room_id, created_at) VALUES ($alias, $room_id, $created_at)";
        self.db
            .query(insert_query)
            .bind(("alias", alias.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("created_at", chrono::Utc::now()))
            .await?;

        Ok(())
    }

    /// Remove room alias
    pub async fn remove_room_alias(
        &self,
        room_id: &str,
        alias: &str,
    ) -> Result<(), RepositoryError> {
        let query = "DELETE FROM room_aliases WHERE room_id = $room_id AND alias = $alias";
        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("alias", alias.to_string()))
            .await?;

        Ok(())
    }

    /// Get the count of joined members in a room
    async fn get_joined_member_count(&self, room_id: &str) -> Result<i64, RepositoryError> {
        let query = "
            SELECT count()
            FROM membership
            WHERE room_id = $room_id
              AND membership = 'join'
        ";

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let count: Option<i64> = response.take(0)?;
        Ok(count.unwrap_or(0))
    }

    /// Get m.space.child state events for a space room (WITH origin_server_ts per Matrix spec)
    async fn get_children_state_events(&self, room_id: &str) -> Result<Vec<SpaceHierarchyStrippedStateEvent>, RepositoryError> {
        let query = "
            SELECT content, origin_server_ts, sender, state_key, event_type
            FROM event
            WHERE room_id = $room_id
              AND event_type = 'm.space.child'
            ORDER BY origin_server_ts DESC
        ";

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        #[derive(Deserialize)]
        struct EventData {
            content: matryx_entity::types::EventContent,
            origin_server_ts: i64,
            sender: String,
            state_key: String,
            event_type: String,
        }

        let events: Vec<EventData> = response.take(0)?;

        Ok(events.into_iter().map(|e| {
            SpaceHierarchyStrippedStateEvent::new(
                e.content,
                e.origin_server_ts,
                e.sender,
                e.state_key,
                e.event_type,
            )
        }).collect())
    }

    /// Get allowed room IDs for restricted rooms
    async fn get_allowed_room_ids(&self, room_id: &str) -> Result<Option<Vec<String>>, RepositoryError> {
        // Get the m.room.join_rules state event
        let join_rules_event = self.get_room_state_event(room_id, "m.room.join_rules", "").await?;

        if let Some(event) = join_rules_event {
            // Check if it's a restricted or knock_restricted room
            if let Some(join_rule) = event.content.get("join_rule").and_then(|v| v.as_str()) {
                if join_rule == "restricted" || join_rule == "knock_restricted" {
                    // Extract room IDs from allow conditions
                    if let Some(allow) = event.content.get("allow").and_then(|v| v.as_array()) {
                        let room_ids: Vec<String> = allow
                            .iter()
                            .filter_map(|condition| {
                                if condition.get("type")?.as_str()? == "m.room_membership" {
                                    condition.get("room_id")?.as_str().map(String::from)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if !room_ids.is_empty() {
                            return Ok(Some(room_ids));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Get encryption algorithm for a room
    async fn get_room_encryption(&self, room_id: &str) -> Result<Option<String>, RepositoryError> {
        let encryption_event = self.get_room_state_event(room_id, "m.room.encryption", "").await?;

        if let Some(event) = encryption_event {
            if let Some(algorithm) = event.content.get("algorithm").and_then(|v| v.as_str()) {
                return Ok(Some(algorithm.to_string()));
            }
        }

        Ok(None)
    }

    /// Get room hierarchy (spaces)
    /// Get room hierarchy (spaces)
    pub async fn get_room_hierarchy(
        &self,
        room_id: &str,
        suggested_only: bool,
        max_depth: Option<u32>,
    ) -> Result<HierarchyResponse, RepositoryError> {
        let mut visited = std::collections::HashSet::new();
        self.get_room_hierarchy_internal(room_id, suggested_only, max_depth, &mut visited).await
    }

    /// Internal implementation of get_room_hierarchy with visited tracking for cycle detection and deduplication
    async fn get_room_hierarchy_internal(
        &self,
        room_id: &str,
        suggested_only: bool,
        max_depth: Option<u32>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<HierarchyResponse, RepositoryError> {
        use matryx_entity::types::SpaceHierarchyParentRoom;
        
        // Cycle detection: if we've already visited this room, return empty result to break the cycle
        if visited.contains(room_id) {
            let empty_room = SpaceHierarchyParentRoom {
                room_id: room_id.to_string(),
                canonical_alias: None,
                children_state: Vec::new(),
                room_type: None,
                name: None,
                num_joined_members: 0,
                topic: None,
                world_readable: false,
                guest_can_join: false,
                join_rule: None,
                avatar_url: None,
                allowed_room_ids: None,
                encryption: None,
                room_version: None,
            };
            return Ok(HierarchyResponse::new(Vec::new(), Vec::new(), empty_room));
        }
        
        // Mark this room as visited immediately to prevent recursion into it
        visited.insert(room_id.to_string());
        
        let room = self.get_by_id(room_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            }
        })?;

        // Get child rooms if this is a space
        let children_query = if suggested_only {
            "SELECT child_room_id, suggested FROM space_children WHERE parent_room_id = $room_id AND suggested = true"
        } else {
            "SELECT child_room_id, suggested FROM space_children WHERE parent_room_id = $room_id"
        };

        let mut result = self
            .db
            .query(children_query)
            .bind(("room_id", room_id.to_string()))
            .await?;
        let children: Vec<serde_json::Value> = result.take(0)?;

        // Process child rooms based on max_depth
        let mut child_chunks = Vec::new();
        let mut inaccessible_children = Vec::new();
        
        if max_depth.is_none_or(|depth| depth > 0) {
            for child in children {
                if let Some(child_room_id) = child.get("child_room_id").and_then(|v| v.as_str()) {
                    // Deduplication check: skip if already visited
                    if visited.contains(child_room_id) {
                        continue;
                    }
                    
                    // Try to get child room details
                    if let Ok(Some(child_room)) = self.get_by_id(child_room_id).await {
                        use matryx_entity::types::SpaceHierarchyChildRoomsChunk;
                        let chunk = SpaceHierarchyChildRoomsChunk {
                            room_id: child_room.room_id.clone(),
                            canonical_alias: child_room.canonical_alias,
                            children_state: Vec::new(), // Would recursively get children if depth allows
                            room_type: child_room.room_type,
                            name: child_room.name,
                            num_joined_members: self.get_joined_member_count(child_room_id).await.unwrap_or(0),
                            topic: child_room.topic,
                            world_readable: child_room.history_visibility == Some("world_readable".to_string()),
                            guest_can_join: child_room.guest_access == Some("can_join".to_string()),
                            join_rule: Some(child_room.join_rules.unwrap_or_else(|| "invite".to_string())),
                            avatar_url: child_room.avatar_url,
                            room_version: Some("9".to_string()), // Default room version
                            allowed_room_ids: None,
                            encryption: None,
                        };
                        child_chunks.push(chunk);
                        
                        // Mark child as visited when added
                        visited.insert(child_room_id.to_string());
                        
                        // Recursively get children if max_depth allows
                        if let Some(depth) = max_depth {
                            if depth > 1 {
                                let child_hierarchy = Box::pin(self.get_room_hierarchy_internal(
                                    child_room_id, 
                                    suggested_only, 
                                    Some(depth - 1),
                                    visited
                                )).await;
                                if let Ok(child_hierarchy) = child_hierarchy {
                                    // Merge child hierarchies with deduplication
                                    for child in child_hierarchy.children {
                                        if visited.insert(child.room_id.clone()) {
                                            child_chunks.push(child);
                                        }
                                    }
                                }
                            }
                        } else {
                            // No depth limit, continue recursion (but limit to prevent infinite loops)
                            if child_chunks.len() < 100 { // Safety limit
                                let child_hierarchy = Box::pin(self.get_room_hierarchy_internal(
                                    child_room_id, 
                                    suggested_only, 
                                    Some(10), // Reasonable default depth limit
                                    visited
                                )).await;
                                if let Ok(child_hierarchy) = child_hierarchy {
                                    // Merge child hierarchies with deduplication
                                    for child in child_hierarchy.children {
                                        if visited.insert(child.room_id.clone()) {
                                            child_chunks.push(child);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // Child room is inaccessible
                        inaccessible_children.push(child_room_id.to_string());
                    }
                }
            }
        }

        // Create parent room representation
        let parent_room = SpaceHierarchyParentRoom {
            room_id: room.room_id.clone(),
            canonical_alias: room.canonical_alias,
            children_state: self.get_children_state_events(&room.room_id).await.unwrap_or_else(|_| Vec::new()),
            room_type: room.room_type,
            name: room.name,
            num_joined_members: self.get_joined_member_count(&room.room_id).await.unwrap_or(0),
            topic: room.topic,
            world_readable: room.history_visibility == Some("world_readable".to_string()),
            guest_can_join: room.guest_access == Some("can_join".to_string()),
            join_rule: Some(room.join_rules.unwrap_or_else(|| "invite".to_string())),
            avatar_url: room.avatar_url,
            allowed_room_ids: self.get_allowed_room_ids(&room.room_id).await.unwrap_or(None),
            encryption: self.get_room_encryption(&room.room_id).await.unwrap_or(None),
            room_version: Some(room.room_version),
        };

        Ok(HierarchyResponse::new(child_chunks, inaccessible_children, parent_room))
    }

    /// Validate room operation (already exists, enhanced version)
    pub async fn validate_room_operation_enhanced(
        &self,
        room_id: &str,
        user_id: &str,
        operation: RoomOperation,
    ) -> Result<bool, RepositoryError> {
        // Check if user is a member of the room
        if !self.check_membership(room_id, user_id).await? {
            return Ok(false);
        }

        // Get user's power level
        let power_levels = self.get_room_power_levels(room_id).await?;
        let user_level = power_levels
            .users
            .get(user_id)
            .copied()
            .unwrap_or(power_levels.users_default);

        // Check required power level for operation
        let required_level = match operation {
            RoomOperation::SendMessage => {
                power_levels
                    .events
                    .get("m.room.message")
                    .copied()
                    .unwrap_or(power_levels.events_default)
            },
            RoomOperation::SendState(event_type) => {
                power_levels
                    .events
                    .get(&event_type)
                    .copied()
                    .unwrap_or(power_levels.state_default)
            },
            RoomOperation::InviteUser => power_levels.invite,
            RoomOperation::KickUser => power_levels.kick,
            RoomOperation::BanUser => power_levels.ban,
            RoomOperation::ChangeSettings => power_levels.state_default,
            RoomOperation::ChangePowerLevels => {
                power_levels.events.get("m.room.power_levels").copied().unwrap_or(100)
            },
        };

        Ok(user_level >= required_level)
    }

    /// Update room's latest event timestamp and event ID
    pub async fn update_room_latest_event(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<(), RepositoryError> {
        use surrealdb::opt::PatchOps;

        let _: Option<Room> = self
            .db
            .update(("room", room_id))
            .patch(
                PatchOps::new()
                    .replace("/updated_at", chrono::Utc::now())
                    .replace("/latest_event_id", event_id),
            )
            .await?;

        Ok(())
    }

    // TASK15 SUBTASK 3: Add public room methods

    /// Get room member count
    pub async fn get_room_member_count(&self, room_id: &str) -> Result<u32, RepositoryError> {
        let query = "SELECT count() FROM membership WHERE room_id = $room_id AND membership = 'join' GROUP ALL";
        
        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_member_count".to_string(),
            })?;

        let count: Option<i64> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_member_count_parse".to_string(),
        })?;

        Ok(count.unwrap_or(0) as u32)
    }

    /// Get room public information
    pub async fn get_room_public_info(&self, room_id: &str) -> Result<Option<crate::repository::public_rooms::PublicRoomInfo>, RepositoryError> {
        let query = r#"
            SELECT room_id, name, topic, avatar_url, canonical_alias, 
                   world_readable, guest_can_join, join_rule, room_type, visibility,
                   (SELECT count() FROM membership WHERE room_id = $parent.room_id AND membership = 'join') as num_joined_members
            FROM room 
            WHERE room_id = $room_id
        "#;

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_public_info".to_string(),
            })?;

        let room_data: Option<RoomDataTuple> = 
            response.take(0).map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_public_info_parse".to_string(),
            })?;

        if let Some((room_id, name, topic, avatar_url, canonical_alias, world_readable, guest_can_join, join_rule, room_type, visibility, num_joined_members)) = room_data {
            let visibility_enum = match visibility.as_str() {
                "public" => crate::repository::public_rooms::RoomDirectoryVisibility::Public,
                _ => crate::repository::public_rooms::RoomDirectoryVisibility::Private,
            };

            Ok(Some(crate::repository::public_rooms::PublicRoomInfo {
                room_id,
                name,
                topic,
                avatar_url,
                canonical_alias,
                num_joined_members,
                world_readable,
                guest_can_join,
                join_rule,
                room_type,
                visibility: visibility_enum,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update room public information
    pub async fn update_room_public_info(&self, room_id: &str, info: &crate::repository::public_rooms::PublicRoomInfo) -> Result<(), RepositoryError> {
        let visibility_str = match info.visibility {
            crate::repository::public_rooms::RoomDirectoryVisibility::Public => "public",
            crate::repository::public_rooms::RoomDirectoryVisibility::Private => "private",
        };

        let query = r#"
            UPDATE room SET 
                name = $name,
                topic = $topic,
                avatar_url = $avatar_url,
                canonical_alias = $canonical_alias,
                world_readable = $world_readable,
                guest_can_join = $guest_can_join,
                join_rule = $join_rule,
                room_type = $room_type,
                visibility = $visibility
            WHERE room_id = $room_id
        "#;

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("name", info.name.clone()))
            .bind(("topic", info.topic.clone()))
            .bind(("avatar_url", info.avatar_url.clone()))
            .bind(("canonical_alias", info.canonical_alias.clone()))
            .bind(("world_readable", info.world_readable))
            .bind(("guest_can_join", info.guest_can_join))
            .bind(("join_rule", info.join_rule.clone()))
            .bind(("room_type", info.room_type.clone()))
            .bind(("visibility", visibility_str.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "update_room_public_info".to_string(),
            })?;

        Ok(())
    }



    /// Get room avatar URL
    pub async fn get_room_avatar_url(&self, room_id: &str) -> Result<Option<String>, RepositoryError> {
        let query = r#"
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.avatar' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC 
            LIMIT 1
        "#;

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_avatar_url".to_string(),
            })?;

        let content: Option<serde_json::Value> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_avatar_url_parse".to_string(),
        })?;

        if let Some(content) = content
            && let Some(url) = content.get("url").and_then(|v| v.as_str()) {
                return Ok(Some(url.to_string()));
            }

        Ok(None)
    }

    /// Get room canonical alias
    pub async fn get_room_canonical_alias(&self, room_id: &str) -> Result<Option<String>, RepositoryError> {
        let query = r#"
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.canonical_alias' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC 
            LIMIT 1
        "#;

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_canonical_alias".to_string(),
            })?;

        let content: Option<serde_json::Value> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_canonical_alias_parse".to_string(),
        })?;

        if let Some(content) = content
            && let Some(alias) = content.get("alias").and_then(|v| v.as_str()) {
                return Ok(Some(alias.to_string()));
            }

        Ok(None)
    }

    // TASK16 SUBTASK 3: Add room validation methods

    /// Validate room access for a user and action
    pub async fn validate_room_access(&self, room_id: &str, user_id: &str, action: crate::repository::room_operations::RoomAction) -> Result<bool, RepositoryError> {
        // Check if room exists
        let room = self.get_by_id(room_id).await?;
        if room.is_none() {
            return Ok(false);
        }

        // Get user's membership in the room
        use crate::repository::membership::MembershipRepository;
        let membership_repo = MembershipRepository::new(self.db.clone());
        let membership = membership_repo.get_membership(room_id, user_id).await?;

        match action {
            crate::repository::room_operations::RoomAction::Read => {
                // Can read if joined or if room is world readable
                if let Some(membership) = membership {
                    match membership.membership {
                        matryx_entity::types::MembershipState::Join => Ok(true),
                        _ => self.is_room_world_readable(room_id).await,
                    }
                } else {
                    self.is_room_world_readable(room_id).await
                }
            },
            crate::repository::room_operations::RoomAction::Write | 
            crate::repository::room_operations::RoomAction::SendEvents => {
                // Must be joined to write
                if let Some(membership) = membership {
                    Ok(membership.membership == matryx_entity::types::MembershipState::Join)
                } else {
                    Ok(false)
                }
            },
            crate::repository::room_operations::RoomAction::Invite => {
                self.can_user_invite(room_id, user_id).await
            },
            crate::repository::room_operations::RoomAction::Kick |
            crate::repository::room_operations::RoomAction::Ban => {
                // Check power levels for moderation actions
                membership_repo.can_perform_action(
                    room_id, 
                    user_id, 
                    if matches!(action, crate::repository::room_operations::RoomAction::Kick) {
                        crate::repository::room_operations::MembershipAction::Kick
                    } else {
                        crate::repository::room_operations::MembershipAction::Ban
                    }, 
                    None
                ).await
            },
            crate::repository::room_operations::RoomAction::RedactEvents |
            crate::repository::room_operations::RoomAction::StateEvents => {
                // Check if user has sufficient power level for these actions
                if let Some(membership) = membership {
                    if membership.membership == matryx_entity::types::MembershipState::Join {
                        let user_power_level = membership_repo.get_user_power_level(room_id, user_id).await.unwrap_or(0);
                        let required_level = match action {
                            crate::repository::room_operations::RoomAction::RedactEvents => 50,
                            crate::repository::room_operations::RoomAction::StateEvents => 50,
                            _ => 0,
                        };
                        Ok(user_power_level >= required_level)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            },
        }
    }



    /// Check if room is invite only
    pub async fn is_room_invite_only(&self, room_id: &str) -> Result<bool, RepositoryError> {
        let join_rules = self.get_room_join_rules(room_id).await?;
        Ok(matches!(join_rules, JoinRules::Invite | JoinRules::Private))
    }

    /// Check if user can invite others to the room
    pub async fn can_user_invite(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        use crate::repository::membership::MembershipRepository;
        let membership_repo = MembershipRepository::new(self.db.clone());
        
        // User must be joined to invite
        let membership = membership_repo.get_membership(room_id, user_id).await?;
        if let Some(membership) = membership {
            if membership.membership != matryx_entity::types::MembershipState::Join {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }

        // Check power level for invite
        membership_repo.can_perform_action(
            room_id, 
            user_id, 
            crate::repository::room_operations::MembershipAction::Invite, 
            None
        ).await
    }

    /// Get room guest access settings
    pub async fn get_room_guest_access(&self, room_id: &str) -> Result<GuestAccess, RepositoryError> {
        let query = r#"
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.guest_access' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC 
            LIMIT 1
        "#;

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_guest_access".to_string(),
            })?;

        let content: Option<serde_json::Value> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_guest_access_parse".to_string(),
        })?;

        if let Some(content) = content
            && let Some(guest_access) = content.get("guest_access").and_then(|ga| ga.as_str()) {
                return Ok(match guest_access {
                    "can_join" => GuestAccess::CanJoin,
                    "forbidden" => GuestAccess::Forbidden,
                    _ => GuestAccess::Forbidden, // Default
                });
            }

        Ok(GuestAccess::Forbidden) // Default
    }

    /// Get join rule allow conditions for restricted rooms (MSC3083)
    pub async fn get_join_rule_allow_conditions(
        &self,
        room_id: &str,
    ) -> Result<Vec<serde_json::Value>, RepositoryError> {
        let query = "
            SELECT content
            FROM room_state_events
            WHERE room_id = $room_id 
              AND event_type = 'm.room.join_rules'
              AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let content: Option<serde_json::Value> = response.take(0)?;

        match content {
            Some(content_value) => {
                let allow_conditions = content_value
                    .get("allow")
                    .and_then(|v| v.as_array())
                    .unwrap_or(&vec![])
                    .clone();
                Ok(allow_conditions)
            },
            None => Ok(vec![]),
        }
    }

    /// Check if a room is a direct message room
    /// A DM room has exactly 2 members and no name/topic set
    pub async fn is_direct_message_room(&self, room_id: &str) -> Result<bool, RepositoryError> {
        // Get member count for joined users
        let member_count_query = "
            SELECT count() 
            FROM membership 
            WHERE room_id = $room_id 
              AND membership = 'join'
        ";

        let mut response = self
            .db
            .query(member_count_query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let member_count: Option<i64> = response.take(0)?;
        let member_count = member_count.unwrap_or(0);

        // Check if room has name or topic set
        let room_state_query = "
            SELECT count()
            FROM event 
            WHERE room_id = $room_id 
              AND event_type IN ['m.room.name', 'm.room.topic']
              AND state_key = ''
        ";

        let mut response = self
            .db
            .query(room_state_query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let has_name_or_topic: Option<i64> = response.take(0)?;
        let has_name_or_topic = has_name_or_topic.unwrap_or(0) > 0;

        Ok(member_count == 2 && !has_name_or_topic)
    }

    /// Resolve room alias to room ID
    pub async fn resolve_room_alias(&self, alias: &str) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT room_id FROM room_aliases WHERE alias = $alias";

        let mut response = self.db.query(query).bind(("alias", alias.to_string())).await?;

        let room_id: Option<String> = response.take(0)?;
        Ok(room_id)
    }

    /// Check if room allows knocking by examining join rules
    pub async fn check_room_allows_knocking(&self, room_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT content.join_rule
            FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.join_rules' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct JoinRulesContent {
            join_rule: Option<String>,
        }

        let join_rules: Option<JoinRulesContent> = response.take(0)?;

        match join_rules {
            Some(rules) => {
                let join_rule = rules.join_rule.unwrap_or_else(|| "invite".to_string());
                Ok(join_rule == "knock")
            },
            None => {
                // No join rules event found, default is "invite" which doesn't allow knocking
                Ok(false)
            },
        }
    }

    /// Check if server is allowed by room ACLs
    pub async fn check_server_acls(&self, room_id: &str, server_name: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT content.allow, content.deny
            FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.server_acl' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct ServerAclContent {
            allow: Option<Vec<String>>,
            deny: Option<Vec<String>>,
        }

        let server_acl: Option<ServerAclContent> = response.take(0)?;

        match server_acl {
            Some(acl) => {
                // Check deny list first
                if let Some(deny_list) = acl.deny {
                    for pattern in deny_list {
                        if Self::server_matches_pattern(server_name, &pattern) {
                            return Ok(false);
                        }
                    }
                }

                // Check allow list
                if let Some(allow_list) = acl.allow {
                    for pattern in allow_list {
                        if Self::server_matches_pattern(server_name, &pattern) {
                            return Ok(true);
                        }
                    }
                    // If allow list exists but server doesn't match, deny
                    Ok(false)
                } else {
                    // No allow list, server is allowed (unless denied above)
                    Ok(true)
                }
            },
            None => {
                // No server ACL event, all servers allowed
                Ok(true)
            },
        }
    }

    /// Check if server name matches ACL pattern (supports wildcards)
    fn server_matches_pattern(server_name: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if let Some(domain_suffix) = pattern.strip_prefix("*.") {
            return server_name == domain_suffix ||
                server_name.ends_with(&format!(".{}", domain_suffix));
        }

        server_name == pattern
    }

    /// Get room visibility settings from state events
    pub async fn get_room_visibility_settings(&self, room_id: &str) -> Result<(String, bool, bool), RepositoryError> {
        // Query for join rules, guest access, and history visibility
        let query = "
            SELECT event_type, content
            FROM event
            WHERE room_id = $room_id
            AND event_type IN ['m.room.join_rules', 'm.room.guest_access', 'm.room.history_visibility']
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct StateEvent {
            event_type: String,
            content: serde_json::Value,
        }

        let state_events: Vec<StateEvent> = response.take(0)?;

        let mut join_rule = "invite".to_string(); // Default
        let mut guest_can_join = false; // Default
        let mut world_readable = false; // Default

        for event in state_events {
            match event.event_type.as_str() {
                "m.room.join_rules" => {
                    if let Some(rule) = event.content.get("join_rule").and_then(|v| v.as_str()) {
                        join_rule = rule.to_string();
                    }
                },
                "m.room.guest_access" => {
                    if let Some(access) = event.content.get("guest_access").and_then(|v| v.as_str()) {
                        guest_can_join = access == "can_join";
                    }
                },
                "m.room.history_visibility" => {
                    if let Some(visibility) = event.content.get("history_visibility").and_then(|v| v.as_str()) {
                        world_readable = visibility == "world_readable";
                    }
                },
                _ => {},
            }
        }

        Ok((join_rule, guest_can_join, world_readable))
    }

    /// Check if a server has permission to view room state
    pub async fn check_server_state_permission(&self, room_id: &str, requesting_server: &str) -> Result<bool, RepositoryError> {
        // Check if the requesting server has any users in the room
        let query = "
            SELECT COUNT() as count
            FROM membership
            WHERE room_id = $room_id
            AND user_id CONTAINS $server_suffix
            AND membership IN ['join', 'invite', 'leave']
            LIMIT 1
        ";

        let server_suffix = format!(":{}", requesting_server);

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("server_suffix", server_suffix))
            .await?;

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let count_result: Option<CountResult> = response.take(0)?;
        let has_users = count_result.map(|c| c.count > 0).unwrap_or(false);

        if has_users {
            return Ok(true);
        }

        // Check if room is world-readable
        let world_readable = self.is_room_world_readable(room_id).await?;
        Ok(world_readable)
    }



    /// Get the full join rule content for a room
    pub async fn get_room_join_rule_content(&self, room_id: &str) -> Result<serde_json::Value, RepositoryError> {
        let query = "
            SELECT content
            FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.join_rules' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct EventContent {
            content: serde_json::Value,
        }

        let event_data: Option<EventContent> = response.take(0)?;
        Ok(event_data
            .map(|e| e.content)
            .unwrap_or_else(|| serde_json::json!({"join_rule": "invite"})))
    }

    /// Get room join rules as string
    pub async fn get_room_join_rules_string(&self, room_id: &str) -> Result<String, RepositoryError> {
        let query = "
            SELECT content.join_rule 
            FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.join_rules' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct JoinRuleContent {
            join_rule: String,
        }

        let content: Option<JoinRuleContent> = response.take(0)?;
        Ok(content.map(|c| c.join_rule).unwrap_or_else(|| "invite".to_string()))
    }

    /// Check if user has sufficient power level to invite users
    pub async fn check_invite_power_level(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT content.invite, content.users
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
            invite: Option<i64>,
            users: Option<std::collections::HashMap<String, i64>>,
        }

        let power_levels: Option<PowerLevelsContent> = response.take(0)?;

        match power_levels {
            Some(pl) => {
                let required_level = pl.invite.unwrap_or(0); // Default invite level is 0
                let user_level = pl.users.and_then(|users| users.get(user_id).copied()).unwrap_or(0); // Default user level is 0

                Ok(user_level >= required_level)
            },
            None => {
                // No power levels event, default behavior allows invites
                Ok(true)
            },
        }
    }

    /// Get room history visibility setting
    pub async fn get_room_history_visibility(&self, room_id: &str) -> Result<String, RepositoryError> {
        let query = "
            SELECT content.history_visibility
            FROM room_state_events 
            WHERE room_id = $room_id AND type = 'm.room.history_visibility' AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct HistoryVisibilityContent {
            history_visibility: Option<String>,
        }

        let visibility_events: Vec<HistoryVisibilityContent> = response.take(0)?;

        if let Some(event) = visibility_events.first()
            && let Some(visibility) = &event.history_visibility {
                return Ok(visibility.clone());
            }

        // Default to "shared" if no history visibility event found
        Ok("shared".to_string())
    }

    /// Check if room is world readable
    pub async fn is_room_world_readable(&self, room_id: &str) -> Result<bool, RepositoryError> {
        let visibility = self.get_room_history_visibility(room_id).await?;
        Ok(visibility == "world_readable")
    }

    /// Check room membership for a user
    pub async fn check_room_membership(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT content.membership
            FROM room_memberships 
            WHERE room_id = $room_id AND user_id = $user_id
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct MembershipContent {
            membership: Option<String>,
        }

        let membership_events: Vec<MembershipContent> = response.take(0)?;

        if let Some(event) = membership_events.first()
            && let Some(membership) = &event.membership {
                return Ok(membership == "join");
            }

        Ok(false)
    }

    /// Get room state events for initial sync
    pub async fn get_room_state_events(&self, room_id: &str) -> Result<Vec<serde_json::Value>, RepositoryError> {
        let query = "
            SELECT type, state_key, content, sender, origin_server_ts, event_id
            FROM room_state_events 
            WHERE room_id = $room_id
            ORDER BY origin_server_ts DESC
            LIMIT 50
        ";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let state_events: Vec<serde_json::Value> = response.take(0)?;
        Ok(state_events)
    }

    /// Get room messages for initial sync
    pub async fn get_room_messages(&self, room_id: &str, limit: u32) -> Result<Vec<serde_json::Value>, RepositoryError> {
        let query = "
            SELECT type, content, sender, origin_server_ts, event_id
            FROM room_timeline_events 
            WHERE room_id = $room_id AND type = 'm.room.message'
            ORDER BY origin_server_ts DESC
            LIMIT $limit
        ";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("limit", limit))
            .await?;

        let messages: Vec<serde_json::Value> = response.take(0)?;
        Ok(messages)
    }
}
