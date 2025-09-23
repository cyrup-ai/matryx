use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Account data sync response
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountDataSync {
    /// Global account data
    pub global: HashMap<String, Value>,

    /// Room-specific account data keyed by room ID
    pub rooms: HashMap<String, HashMap<String, Value>>,

    /// Sync token for next request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_batch: Option<String>,

    /// Events that have been updated since last sync
    pub events: Vec<AccountDataEvent>,
}

/// Account data event for sync
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountDataEvent {
    /// Event type
    pub r#type: String,

    /// Event content
    pub content: Value,

    /// Room ID for room-specific account data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}

impl AccountDataSync {
    /// Create new empty account data sync
    pub fn new() -> Self {
        Self {
            global: HashMap::new(),
            rooms: HashMap::new(),
            next_batch: None,
            events: Vec::new(),
        }
    }

    /// Create with initial data
    pub fn with_data(
        global: HashMap<String, Value>,
        rooms: HashMap<String, HashMap<String, Value>>,
    ) -> Self {
        Self {
            global,
            rooms,
            next_batch: None,
            events: Vec::new(),
        }
    }

    /// Add global account data
    pub fn add_global_data(&mut self, data_type: String, content: Value) {
        self.global.insert(data_type, content);
    }

    /// Add room account data
    pub fn add_room_data(&mut self, room_id: String, data_type: String, content: Value) {
        self.rooms.entry(room_id).or_default().insert(data_type, content);
    }

    /// Add account data event
    pub fn add_event(&mut self, event: AccountDataEvent) {
        self.events.push(event);
    }
}

impl Default for AccountDataSync {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountDataEvent {
    /// Create new account data event
    pub fn new(r#type: String, content: Value, room_id: Option<String>) -> Self {
        Self { r#type, content, room_id, timestamp: Utc::now() }
    }

    /// Create global account data event
    pub fn global(r#type: String, content: Value) -> Self {
        Self::new(r#type, content, None)
    }

    /// Create room account data event
    pub fn room(r#type: String, content: Value, room_id: String) -> Self {
        Self::new(r#type, content, Some(room_id))
    }
}
