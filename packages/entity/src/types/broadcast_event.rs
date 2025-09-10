use crate::types::EventContent;
use serde::{Deserialize, Serialize};

/// Broadcast event for PDUs
/// Represents a Matrix event that is broadcast across homeservers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastEvent {
    /// Event content
    pub content: EventContent,
    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,
    /// Room ID where the event occurred
    pub room_id: String,
    /// User ID of the sender
    pub sender: String,
    /// Event ID
    pub event_id: String,
    /// Origin server timestamp
    pub origin_server_ts: i64,
}

impl BroadcastEvent {
    pub fn new(
        content: EventContent,
        event_type: String,
        room_id: String,
        sender: String,
        event_id: String,
        origin_server_ts: i64,
    ) -> Self {
        Self {
            content,
            event_type,
            room_id,
            sender,
            event_id,
            origin_server_ts,
        }
    }
}
