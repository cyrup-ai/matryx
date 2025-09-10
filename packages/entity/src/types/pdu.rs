use crate::types::{EventContent, UnsignedData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Persistent Data Unit - core Matrix federation event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PDU {
    /// Event content
    pub content: EventContent,

    /// Event ID
    pub event_id: String,

    /// Origin server timestamp
    pub origin_server_ts: i64,

    /// Room ID
    pub room_id: String,

    /// Sender user ID
    pub sender: String,

    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,

    /// State key for state events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,

    /// Previous event IDs
    pub prev_events: Vec<String>,
    /// Authorization events
    pub auth_events: Vec<String>,

    /// Depth in the event graph
    pub depth: i64,

    /// Event signatures
    pub signatures: HashMap<String, HashMap<String, String>>,

    /// Event hashes
    pub hashes: HashMap<String, String>,

    /// Unsigned data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<UnsignedData>,
}

impl PDU {
    pub fn new(
        content: EventContent,
        event_id: String,
        origin_server_ts: i64,
        room_id: String,
        sender: String,
        event_type: String,
        prev_events: Vec<String>,
        auth_events: Vec<String>,
        depth: i64,
    ) -> Self {
        Self {
            content,
            event_id,
            origin_server_ts,
            room_id,
            sender,
            event_type,
            state_key: None,
            prev_events,
            auth_events,
            depth,
            signatures: HashMap::new(),
            hashes: HashMap::new(),
            unsigned: None,
        }
    }
}
