use crate::types::{EventContent, UnsignedData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameters for creating a PDU
#[derive(Debug, Clone)]
pub struct PduParams {
    pub content: EventContent,
    pub event_id: String,
    pub origin_server_ts: i64,
    pub room_id: String,
    pub sender: String,
    pub event_type: String,
    pub prev_events: Vec<String>,
    pub auth_events: Vec<String>,
    pub depth: i64,
}

/// Persistent Data Unit (PDU) - Matrix federation event structure
///
/// PDUs represent room events that are stored permanently in the room's history.
/// They are the fundamental building blocks of Matrix rooms and are exchanged
/// between homeservers during federation to maintain consistent room state.
///
/// **Matrix Specification:** Server-Server API Event Format
/// **Purpose:** Permanent room events (messages, state changes, etc.)
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
    pub fn new(params: PduParams) -> Self {
        Self {
            content: params.content,
            event_id: params.event_id,
            origin_server_ts: params.origin_server_ts,
            room_id: params.room_id,
            sender: params.sender,
            event_type: params.event_type,
            state_key: None,
            prev_events: params.prev_events,
            auth_events: params.auth_events,
            depth: params.depth,
            signatures: HashMap::new(),
            hashes: HashMap::new(),
            unsigned: None,
        }
    }
}
