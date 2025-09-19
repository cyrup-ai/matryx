use crate::types::EventContent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameters for creating a PDU event
#[derive(Debug, Clone)]
pub struct PduParams {
    pub event_id: String,
    pub sender: String,
    pub origin_server_ts: i64,
    pub event_type: String,
    pub room_id: String,
    pub content: EventContent,
    pub auth_events: Vec<String>,
    pub depth: i64,
    pub prev_events: Vec<String>,
}

/// Event - Matrix spec compliant PDU (Persistent Data Unit)
/// Source: Matrix server-server API specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Event {
    /// Unique event identifier
    pub event_id: String,

    /// Event sender user ID
    pub sender: String,

    /// Server timestamp when event was created
    pub origin_server_ts: i64,

    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,

    /// Room this event belongs to
    pub room_id: String,

    /// Event content
    pub content: EventContent,

    /// State key for state events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,

    /// Unsigned event metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<serde_json::Value>,

    /// Authorization events that give sender permission to send this event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_events: Option<Vec<String>>,

    /// Depth in the event DAG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,

    /// Content hashes for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashes: Option<HashMap<String, String>>,

    /// Previous events in the DAG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_events: Option<Vec<String>>,

    /// Digital signatures from servers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,

    /// Whether this event failed soft failure checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_failed: Option<bool>,

    /// Timestamp when event was received by this server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_ts: Option<i64>,

    /// Whether this event is an outlier (not part of room state)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlier: Option<bool>,

    /// Event ID that this event redacts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacts: Option<String>,

    /// Reason why this event was rejected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_reason: Option<String>,
}

impl Event {
    pub fn new(
        event_id: String,
        sender: String,
        origin_server_ts: i64,
        event_type: String,
        room_id: String,
        content: EventContent,
    ) -> Self {
        Self {
            event_id,
            sender,
            origin_server_ts,
            event_type,
            room_id,
            content,
            state_key: None,
            unsigned: None,
            auth_events: None,
            depth: None,
            hashes: None,
            prev_events: None,
            signatures: None,
            soft_failed: None,
            received_ts: None,
            outlier: None,
            redacts: None,
            rejected_reason: None,
        }
    }

    pub fn new_pdu(params: PduParams) -> Self {
        Self {
            event_id: params.event_id,
            sender: params.sender,
            origin_server_ts: params.origin_server_ts,
            event_type: params.event_type,
            room_id: params.room_id,
            content: params.content,
            state_key: None,
            unsigned: None,
            auth_events: Some(params.auth_events),
            depth: Some(params.depth),
            hashes: None,
            prev_events: Some(params.prev_events),
            signatures: None,
            soft_failed: None,
            received_ts: None,
            outlier: None,
            redacts: None,
            rejected_reason: None,
        }
    }
}
