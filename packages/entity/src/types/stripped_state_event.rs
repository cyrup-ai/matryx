use crate::types::EventContent;
use serde::{Deserialize, Serialize};

/// Stripped state event for Matrix room state
/// Represents a minimal state event with only essential fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrippedStateEvent {
    pub content: EventContent,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl StrippedStateEvent {
    pub fn new(
        content: EventContent,
        sender: String,
        state_key: String,
        event_type: String,
    ) -> Self {
        Self { content, sender, state_key, event_type }
    }
}
