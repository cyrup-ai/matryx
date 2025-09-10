use crate::types::PDU;
use serde::{Deserialize, Serialize};

/// Room state for send join responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendJoinRoomState {
    /// State events as PDUs
    pub state: Vec<PDU>,

    /// Auth chain events as PDUs
    pub auth_chain: Vec<PDU>,
}

impl SendJoinRoomState {
    pub fn new(state: Vec<PDU>, auth_chain: Vec<PDU>) -> Self {
        Self { state, auth_chain }
    }
}
