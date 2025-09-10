use crate::types::EventContent;
use serde::{Deserialize, Serialize};

/// SpaceHierarchyStrippedStateEvent
/// Source: spec/server/13-public-md:312-320
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHierarchyStrippedStateEvent {
    pub content: EventContent,
    pub origin_server_ts: i64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl SpaceHierarchyStrippedStateEvent {
    pub fn new(
        content: EventContent,
        origin_server_ts: i64,
        sender: String,
        state_key: String,
        event_type: String,
    ) -> Self {
        Self {
            content,
            origin_server_ts,
            sender,
            state_key,
            event_type,
        }
    }
}
