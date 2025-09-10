use crate::types::{InviteMembershipEventContent, UnsignedData};
use serde::{Deserialize, Serialize};

/// InviteV1Request
/// Source: spec/server/11-room-md:45-70
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteV1Request {
    pub content: InviteMembershipEventContent,
    pub origin: String,
    pub origin_server_ts: i64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub unsigned: UnsignedData,
}

impl InviteV1Request {
    pub fn new(
        content: InviteMembershipEventContent,
        origin: String,
        origin_server_ts: i64,
        sender: String,
        state_key: String,
        event_type: String,
        unsigned: UnsignedData,
    ) -> Self {
        Self {
            content,
            origin,
            origin_server_ts,
            sender,
            state_key,
            event_type,
            unsigned,
        }
    }
}
