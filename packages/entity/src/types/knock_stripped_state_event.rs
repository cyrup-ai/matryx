use crate::types::EventContent;
use serde::{Deserialize, Serialize};

/// Knock stripped state event for Matrix knock operations
/// Represents a minimal state event for knock room operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockStrippedStateEvent {
    pub content: EventContent,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl KnockStrippedStateEvent {
    pub fn new(
        content: EventContent,
        sender: String,
        state_key: String,
        event_type: String,
    ) -> Self {
        Self { content, sender, state_key, event_type }
    }
}
