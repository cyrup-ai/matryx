use crate::types::SendJoinRoomState;
use serde::{Deserialize, Serialize};

/// SendJoinResponse
/// Source: spec/server/09-room-md:174-180
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendJoinResponse {
    pub response: Vec<(i64, SendJoinRoomState)>,
}

impl SendJoinResponse {
    pub fn new(response: Vec<(i64, SendJoinRoomState)>) -> Self {
        Self { response }
    }
}
