use crate::db::generic_dao::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Presence data entity for storing presence information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceData {
    /// Entity ID (auto-generated)
    pub id: Option<String>,
    /// User ID
    pub user_id: String,
    /// Presence state (online, offline, unavailable)
    pub presence: String,
    /// Last active timestamp
    pub last_active_ago: Option<i64>,
    /// Currently active
    pub currently_active: Option<bool>,
    /// Status message
    pub status_msg: Option<String>,
    /// Presence data
    pub data: Value,
    /// Updated at timestamp
    pub updated_at: DateTime<Utc>,
}

impl Entity for PresenceData {
    fn table_name() -> &'static str {
        "presence"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}
