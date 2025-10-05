use crate::repository::error::RepositoryError;
use crate::repository::event::EventRepository;
use std::collections::HashMap;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use matryx_entity::types::{Event, EventContent};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct PowerLevels {
    pub users: HashMap<String, i32>,
    pub users_default: i32,
    pub events: HashMap<String, i32>,
    pub events_default: i32,
    pub state_default: i32,
    pub ban: i32,
    pub kick: i32,
    pub redact: i32,
    pub invite: i32,
}

#[derive(Debug, Clone)]
pub enum PowerLevelAction {
    Ban,
    Kick,
    Invite,
    Redact,
    SendMessage,
    SendState(String), // event_type
    ChangeSettings,
    ChangePowerLevels,
}

pub struct PowerLevelsRepository {
    db: Surreal<Any>,
}

impl PowerLevelsRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Get power levels for a room
    pub async fn get_power_levels(&self, room_id: &str) -> Result<PowerLevels, RepositoryError> {
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

        if let Some(content) = events.first().and_then(|e| e.get("content")) {
                let users = content
                    .get("users")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_i64().map(|i| (k.clone(), i as i32)))
                            .collect()
                    })
                    .unwrap_or_default();

                let events_map = content
                    .get("events")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_i64().map(|i| (k.clone(), i as i32)))
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(PowerLevels {
                    users,
                    users_default: content
                        .get("users_default")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32,
                    events: events_map,
                    events_default: content
                        .get("events_default")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32,
                    state_default: content
                        .get("state_default")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(50) as i32,
                    ban: content.get("ban").and_then(|v| v.as_i64()).unwrap_or(50) as i32,
                    kick: content.get("kick").and_then(|v| v.as_i64()).unwrap_or(50) as i32,
                    redact: content.get("redact").and_then(|v| v.as_i64()).unwrap_or(50) as i32,
                    invite: content.get("invite").and_then(|v| v.as_i64()).unwrap_or(50) as i32,
                });
        }

        // Return default power levels if no event found
        Ok(PowerLevels {
            users: HashMap::new(),
            users_default: 0,
            events: HashMap::new(),
            events_default: 0,
            state_default: 50,
            ban: 50,
            kick: 50,
            redact: 50,
            invite: 50,
        })
    }

    /// Update power levels for a room
    pub async fn update_power_levels(
        &self,
        room_id: &str,
        sender_id: &str,
        new_power_levels: &PowerLevels,
    ) -> Result<(), RepositoryError> {
        // Get current power levels
        let current_power_levels = self.get_power_levels(room_id).await?;

        // Get sender's current power level
        let sender_power = current_power_levels.users
            .get(sender_id)
            .copied()
            .unwrap_or(current_power_levels.users_default);

        // Validate sender can change power levels
        let required_power = current_power_levels.events
            .get("m.room.power_levels")
            .copied()
            .unwrap_or(100); // Default requires power level 100

        if sender_power < required_power {
            return Err(RepositoryError::Forbidden {
                reason: format!(
                    "Insufficient power level to change power levels: {} < {}",
                    sender_power, required_power
                ),
            });
        }

        // Validate each user power level change
        for (user_id, new_level) in &new_power_levels.users {
            let old_level = current_power_levels.users
                .get(user_id)
                .copied()
                .unwrap_or(current_power_levels.users_default);

            // Sender must have power level > old level to demote
            if *new_level < old_level && sender_power <= old_level {
                return Err(RepositoryError::Forbidden {
                    reason: format!(
                        "Cannot demote user {} with equal or higher power level",
                        user_id
                    ),
                });
            }

            // Sender must have power level >= new level to promote
            if *new_level > old_level && sender_power < *new_level {
                return Err(RepositoryError::Forbidden {
                    reason: format!(
                        "Cannot promote user {} above your power level",
                        user_id
                    ),
                });
            }
        }

        // Validate event-specific power level changes
        for (event_type, new_level) in &new_power_levels.events {
            if sender_power < *new_level {
                return Err(RepositoryError::Forbidden {
                    reason: format!(
                        "Cannot set power level for {} above your own level",
                        event_type
                    ),
                });
            }
        }

        // Validate sender isn't increasing their own power level
        if let Some(&new_level) = new_power_levels.users.get(sender_id) {
            if new_level > sender_power {
                return Err(RepositoryError::Forbidden {
                    reason: "Cannot increase own power level".to_string(),
                });
            }
        }

        // Create content for power levels event
        let content = serde_json::json!({
            "users": new_power_levels.users,
            "users_default": new_power_levels.users_default,
            "events": new_power_levels.events,
            "events_default": new_power_levels.events_default,
            "state_default": new_power_levels.state_default,
            "ban": new_power_levels.ban,
            "kick": new_power_levels.kick,
            "redact": new_power_levels.redact,
            "invite": new_power_levels.invite
        });

        // Create new power levels state event
        let event_id = format!("${}", Uuid::new_v4());
        let now = Utc::now();

        let event = Event {
            event_id: event_id.clone(),
            room_id: room_id.to_string(),
            sender: sender_id.to_string(),
            event_type: "m.room.power_levels".to_string(),
            content: EventContent::Unknown(content),
            state_key: Some("".to_string()),
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

        // Create event using EventRepository
        let event_repo = EventRepository::new(self.db.clone());
        event_repo.create(&event).await?;

        Ok(())
    }

    /// Get a user's power level in a room
    pub async fn get_user_power_level(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<i32, RepositoryError> {
        let power_levels = self.get_power_levels(room_id).await?;

        // Check if user has specific power level
        if let Some(level) = power_levels.users.get(user_id) {
            return Ok(*level);
        }

        // Return default power level
        Ok(power_levels.users_default)
    }

    /// Check if a user can perform a specific action
    pub async fn can_user_perform_action(
        &self,
        room_id: &str,
        user_id: &str,
        action: PowerLevelAction,
    ) -> Result<bool, RepositoryError> {
        let user_level = self.get_user_power_level(room_id, user_id).await?;
        let power_levels = self.get_power_levels(room_id).await?;

        let required_level = match action {
            PowerLevelAction::Ban => power_levels.ban,
            PowerLevelAction::Kick => power_levels.kick,
            PowerLevelAction::Invite => power_levels.invite,
            PowerLevelAction::Redact => power_levels.redact,
            PowerLevelAction::SendMessage => power_levels.events_default,
            PowerLevelAction::SendState(event_type) => {
                power_levels
                    .events
                    .get(&event_type)
                    .copied()
                    .unwrap_or(power_levels.state_default)
            },
            PowerLevelAction::ChangeSettings => power_levels.state_default,
            PowerLevelAction::ChangePowerLevels => {
                power_levels.events.get("m.room.power_levels").copied().unwrap_or(100) // Default high level for power level changes
            },
        };

        Ok(user_level >= required_level)
    }

    /// Validate a power level change
    pub async fn validate_power_level_change(
        &self,
        room_id: &str,
        user_id: &str,
        target_user: &str,
        new_level: i32,
    ) -> Result<bool, RepositoryError> {
        let user_level = self.get_user_power_level(room_id, user_id).await?;
        let target_current_level = self.get_user_power_level(room_id, target_user).await?;

        // User must have higher power level than both current and new target levels
        // and must be able to change power levels
        let can_change_power_levels = self
            .can_user_perform_action(room_id, user_id, PowerLevelAction::ChangePowerLevels)
            .await?;

        Ok(can_change_power_levels && user_level > target_current_level && user_level > new_level)
    }
}
