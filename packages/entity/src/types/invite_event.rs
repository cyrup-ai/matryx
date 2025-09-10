use crate::types::{InviteMembershipEventContent, UnsignedData};
use serde::{Deserialize, Serialize};

/// Invite event for Matrix federation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteEvent {
    /// Event content
    pub content: InviteMembershipEventContent,

    /// Event ID
    pub event_id: String,

    /// Origin server timestamp
    pub origin_server_ts: i64,

    /// Room ID
    pub room_id: String,

    /// Sender user ID
    pub sender: String,

    /// State key
    pub state_key: String,

    /// Event type
    #[serde(rename = "type")]
    pub event_type: String,

    /// Unsigned data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<UnsignedData>,
}

impl InviteEvent {
    pub fn new(
        content: InviteMembershipEventContent,
        event_id: String,
        origin_server_ts: i64,
        room_id: String,
        sender: String,
        state_key: String,
        event_type: String,
    ) -> Self {
        Self {
            content,
            event_id,
            origin_server_ts,
            room_id,
            sender,
            state_key,
            event_type,
            unsigned: None,
        }
    }
}
