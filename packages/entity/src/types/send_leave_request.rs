use crate::types::LeaveMembershipEventContent;
use serde::{Deserialize, Serialize};

/// SendLeaveRequest
/// Source: spec/server/10-room-md:108-116
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendLeaveRequest {
    pub content: LeaveMembershipEventContent,
    pub depth: i64,
    pub origin: String,
    pub origin_server_ts: i64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl SendLeaveRequest {
    pub fn new(
        content: LeaveMembershipEventContent,
        depth: i64,
        origin: String,
        origin_server_ts: i64,
        sender: String,
        state_key: String,
        event_type: String,
    ) -> Self {
        Self {
            content,
            depth,
            origin,
            origin_server_ts,
            sender,
            state_key,
            event_type,
        }
    }
}
