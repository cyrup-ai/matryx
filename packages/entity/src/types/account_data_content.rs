use serde::{Deserialize, Serialize};

/// Account data content for Matrix room and global account data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AccountDataContent {
    /// Direct messages mapping (m.direct)
    Direct(std::collections::HashMap<String, Vec<String>>),

    /// Push rules (m.push_rules)
    PushRules(crate::types::Ruleset),

    /// Ignored users list (m.ignored_user_list)
    IgnoredUsers {
        ignored_users: std::collections::HashMap<String, serde_json::Value>,
    },

    /// Room tags (m.tag)
    Tags {
        tags: std::collections::HashMap<String, crate::types::RoomTag>,
    },

    /// Fully read marker (m.fully_read)
    FullyRead { event_id: String },

    /// Generic account data for unknown types
    Generic(std::collections::HashMap<String, serde_json::Value>),
}

impl AccountDataContent {
    pub fn direct(mapping: std::collections::HashMap<String, Vec<String>>) -> Self {
        Self::Direct(mapping)
    }

    pub fn fully_read(event_id: String) -> Self {
        Self::FullyRead { event_id }
    }

    pub fn generic(data: std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self::Generic(data)
    }
}
