use crate::types::{InviteEvent, StrippedStateEvent};
use serde::{Deserialize, Serialize};

/// InviteV2Request
/// Source: spec/server/11-room-md:195-205
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteV2Request {
    pub event: InviteEvent,
    pub invite_room_state: Option<Vec<StrippedStateEvent>>,
    pub room_version: String,
}

impl InviteV2Request {
    pub fn new(
        event: InviteEvent,
        invite_room_state: Option<Vec<StrippedStateEvent>>,
        room_version: String,
    ) -> Self {
        Self { event, invite_room_state, room_version }
    }
}
