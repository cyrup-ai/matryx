use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Matrix room entity as defined in the Matrix specification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Room {
    /// Room ID
    pub room_id: String,

    /// Room version
    pub room_version: String,

    /// Room creator
    pub creator: String,

    /// Room creation timestamp
    pub created_at: DateTime<Utc>,

    /// Room name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Room topic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,

    /// Room avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Room canonical alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_alias: Option<String>,

    /// Room alternative aliases
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt_aliases: Option<Vec<String>>,

    /// Room join rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_rule: Option<String>,

    /// Room history visibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_visibility: Option<String>,

    /// Room guest access
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_access: Option<String>,

    /// Room power levels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_levels: Option<HashMap<String, serde_json::Value>>,

    /// Room encryption settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<HashMap<String, serde_json::Value>>,

    /// Whether the room is federated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub federate: Option<bool>,

    /// Room type (space, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_type: Option<String>,

    /// Whether the room is public
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_public: Option<bool>,

    /// Whether the room is a direct message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_direct: Option<bool>,

    /// Room join rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_rules: Option<String>,

    /// Room tombstone information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tombstone: Option<serde_json::Value>,

    /// Room predecessor information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predecessor: Option<serde_json::Value>,

    /// Current state events count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_events_count: Option<i64>,

    /// Last updated timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

impl Room {
    /// Create a new room
    pub fn new(room_id: String, room_version: String, creator: String) -> Self {
        Self {
            room_id,
            room_version,
            creator,
            created_at: Utc::now(),
            name: None,
            topic: None,
            avatar_url: None,
            canonical_alias: None,
            alt_aliases: None,
            join_rule: Some("invite".to_string()),
            history_visibility: Some("shared".to_string()),
            guest_access: Some("can_join".to_string()),
            power_levels: None,
            encryption: None,
            federate: Some(true),
            room_type: None,
            is_public: Some(false),
            is_direct: Some(false),
            join_rules: Some("invite".to_string()),
            tombstone: None,
            predecessor: None,
            state_events_count: Some(0),
            updated_at: Some(Utc::now()),
        }
    }
}
