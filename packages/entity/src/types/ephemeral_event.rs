use crate::types::EventContent;
use serde::{Deserialize, Serialize};

/// Ephemeral event for EDUs
/// Represents a Matrix ephemeral event that is not persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralEvent {
    /// Event content
    pub content: EventContent,
    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,
    /// Room ID where the event occurred (optional for some ephemeral events)
    pub room_id: Option<String>,
    /// User ID of the sender
    pub sender: String,
}

impl EphemeralEvent {
    pub fn new(
        content: EventContent,
        event_type: String,
        room_id: Option<String>,
        sender: String,
    ) -> Self {
        Self { content, event_type, room_id, sender }
    }
}
