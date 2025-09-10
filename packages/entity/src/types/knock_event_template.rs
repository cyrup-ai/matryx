use crate::types::KnockMembershipEventContent;
use serde::{Deserialize, Serialize};

/// KnockEventTemplate
/// Source: spec/server/12-room-md:72-80
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockEventTemplate {
    pub content: KnockMembershipEventContent,
    pub origin: String,
    pub origin_server_ts: i64,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl KnockEventTemplate {
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
