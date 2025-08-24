use crate::db::generic_dao::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Send queue entry for storing outgoing Matrix requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendQueueEntry {
    /// Entity ID (auto-generated)
    pub id: Option<String>,
    /// Room ID
    pub room_id: String,
    /// Transaction ID
    pub transaction_id: String,
    /// Event type
    pub event_type: String,
    /// Request content
    pub content: Value,
    /// Request priority (higher values = higher priority)
    pub priority: i64,
    /// Error message if the request failed
    pub error: Option<String>,
    /// When this request was created
    pub created_at: DateTime<Utc>,
    /// When this request was last attempted
    pub last_attempted_at: Option<DateTime<Utc>>,
    /// How many times this request has been attempted
    pub attempts: i64,
}

impl Entity for SendQueueEntry {
    fn table_name() -> &'static str {
        "send_queue"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}
