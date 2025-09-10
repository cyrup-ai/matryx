use crate::types::ThirdPartyInviteEventContent;
use serde::{Deserialize, Serialize};

/// ExchangeThirdPartyInviteRequest
/// Source: spec/server/11-room-md:475-485
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeThirdPartyInviteRequest {
    pub content: ThirdPartyInviteEventContent,
    pub room_id: String,
    pub sender: String,
    pub state_key: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl ExchangeThirdPartyInviteRequest {
    pub fn new(
        content: ThirdPartyInviteEventContent,
        room_id: String,
        sender: String,
        state_key: String,
        event_type: String,
    ) -> Self {
        Self { content, room_id, sender, state_key, event_type }
    }
}
