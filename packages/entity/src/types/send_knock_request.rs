use crate::types::KnockMembershipEventContent;
use serde::{Deserialize, Serialize};

/// SendKnockRequest
/// Source: spec/server/12-room-md:200-210
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendKnockRequest {
    pub content: KnockMembershipEventContent,
    pub origin: String,
    pub origin_server_ts: i64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl SendKnockRequest {
    pub fn new(
        content: KnockMembershipEventContent,
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
