use crate::db::generic_dao::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Account data entity for storing account data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDataEntity {
    /// Entity ID (auto-generated)
    pub id: Option<String>,
    /// Event type
    pub event_type: String,
    /// Room ID (None for global account data)
    pub room_id: Option<String>,
    /// Event content
    pub event: Value,
    /// Updated at timestamp
    pub updated_at: DateTime<Utc>,
}

impl Entity for AccountDataEntity {
    fn table_name() -> &'static str {
        "account_data"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}
