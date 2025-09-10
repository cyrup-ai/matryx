use crate::types::MembershipEventContent;
use serde::{Deserialize, Serialize};

/// SendJoinRequest
/// Source: spec/server/09-room-md:151-159
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendJoinRequest {
    pub content: MembershipEventContent,
    pub origin: String,
    pub origin_server_ts: i64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl SendJoinRequest {
    pub fn new(
        content: MembershipEventContent,
        origin: String,
        origin_server_ts: i64,
        sender: String,
        state_key: String,
        event_type: String,
    ) -> Self {
        Self {
            content,
            origin,
            origin_server_ts,
            sender,
            state_key,
            event_type,
        }
    }
}
