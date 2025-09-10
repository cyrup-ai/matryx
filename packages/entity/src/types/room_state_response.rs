use crate::types::PDU;
use serde::{Deserialize, Serialize};

/// RoomStateResponse
/// Source: spec/server/08-room-md:150-181
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomStateResponse {
    pub auth_chain: Vec<PDU>,
    pub pdus: Vec<PDU>,
}

impl RoomStateResponse {
    pub fn new(auth_chain: Vec<PDU>, pdus: Vec<PDU>) -> Self {
        Self { auth_chain, pdus }
    }
}
