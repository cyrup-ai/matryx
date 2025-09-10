use crate::types::KnockStrippedStateEvent;
use serde::{Deserialize, Serialize};

/// SendKnockResponse
/// Source: spec/server/12-room-md:250-255
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendKnockResponse {
    pub knock_room_state: Vec<KnockStrippedStateEvent>,
}

impl SendKnockResponse {
    pub fn new(knock_room_state: Vec<KnockStrippedStateEvent>) -> Self {
        Self { knock_room_state }
    }
}
