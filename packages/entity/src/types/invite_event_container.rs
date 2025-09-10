use crate::types::{InviteEvent, StrippedStateEvent};
use serde::{Deserialize, Serialize};

/// Container for invite events in federation responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteEventContainer {
    /// The invite event
    pub invite_event: InviteEvent,

    /// Stripped state events for the room
    pub invite_room_state: Vec<StrippedStateEvent>,
}

impl InviteEventContainer {
    pub fn new(invite_event: InviteEvent, invite_room_state: Vec<StrippedStateEvent>) -> Self {
        Self { invite_event, invite_room_state }
    }
}
