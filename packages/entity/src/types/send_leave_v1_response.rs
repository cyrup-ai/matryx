use crate::types::SendJoinRoomState;
use serde::{Deserialize, Serialize};

/// SendLeaveV1Response
/// Source: spec/server/10-room-md:130-135
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendLeaveV1Response {
    pub response: Vec<(i64, SendJoinRoomState)>,
}

impl SendLeaveV1Response {
    pub fn new(response: Vec<(i64, SendJoinRoomState)>) -> Self {
        Self { response }
    }
}
