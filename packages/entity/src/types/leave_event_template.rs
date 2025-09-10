use crate::types::LeaveMembershipEventContent;
use serde::{Deserialize, Serialize};

/// LeaveEventTemplate
/// Source: spec/server/10-room-md:45-53
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveEventTemplate {
    pub content: LeaveMembershipEventContent,
    pub origin: String,
    pub origin_server_ts: i64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl LeaveEventTemplate {
    pub fn new(
        content: LeaveMembershipEventContent,
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
