use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Power levels for a Matrix room
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerLevels {
    /// Default power level for users
    #[serde(default = "default_users_default")]
    pub users_default: i64,

    /// Default power level for events
    #[serde(default = "default_events_default")]
    pub events_default: i64,

    /// Power levels for specific users
    #[serde(default)]
    pub users: HashMap<String, i64>,

    /// Power levels for specific event types
    #[serde(default)]
    pub events: HashMap<String, i64>,

    /// Power level required to ban users
    #[serde(default = "default_ban")]
    pub ban: i64,

    /// Power level required to kick users
    #[serde(default = "default_kick")]
    pub kick: i64,

    /// Power level required to redact events
    #[serde(default = "default_redact")]
    pub redact: i64,

    /// Power level required to invite users
    #[serde(default = "default_invite")]
    pub invite: i64,

    /// Power level required to send state events
    #[serde(default = "default_state_default")]
    pub state_default: i64,

    /// Notifications power levels
    #[serde(default)]
    pub notifications: NotificationPowerLevels,
}

/// Notification power levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationPowerLevels {
    /// Power level required for room-wide notifications
    #[serde(default = "default_room")]
    pub room: i64,
}

fn default_users_default() -> i64 { 0 }
fn default_events_default() -> i64 { 0 }
fn default_ban() -> i64 { 50 }
fn default_kick() -> i64 { 50 }
fn default_redact() -> i64 { 50 }
fn default_invite() -> i64 { 50 }
fn default_state_default() -> i64 { 50 }
fn default_room() -> i64 { 50 }

impl Default for NotificationPowerLevels {
    fn default() -> Self {
        Self {
            room: default_room(),
        }
    }
}

impl Default for PowerLevels {
    fn default() -> Self {
        Self {
            users_default: default_users_default(),
            events_default: default_events_default(),
            users: HashMap::new(),
            events: HashMap::new(),
            ban: default_ban(),
            kick: default_kick(),
            redact: default_redact(),
            invite: default_invite(),
            state_default: default_state_default(),
            notifications: NotificationPowerLevels::default(),
        }
    }
}

impl PowerLevels {
    /// Create new power levels with a room creator
    pub fn new_with_creator(creator_user_id: &str) -> Self {
        let mut power_levels = Self::default();
        power_levels.users.insert(creator_user_id.to_string(), 100);
        power_levels
    }

    /// Get power level for a user
    pub fn get_user_level(&self, user_id: &str) -> i64 {
        self.users.get(user_id).copied().unwrap_or(self.users_default)
    }

    /// Get power level required for an event type
    pub fn get_event_level(&self, event_type: &str) -> i64 {
        self.events.get(event_type).copied().unwrap_or(self.events_default)
    }

    /// Check if user can perform an action requiring a certain power level
    pub fn user_can(&self, user_id: &str, required_level: i64) -> bool {
        self.get_user_level(user_id) >= required_level
    }

    /// Check if user can send an event type
    pub fn user_can_send_event(&self, user_id: &str, event_type: &str) -> bool {
        let user_level = self.get_user_level(user_id);
        let required_level = self.get_event_level(event_type);
        user_level >= required_level
    }
}