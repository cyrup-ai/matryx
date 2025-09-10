use crate::types::SignedThirdPartyInvite;
use serde::{Deserialize, Serialize};

/// ThirdPartyInvite
/// Source: spec/server/11-room-md:377-385
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyInvite {
    pub address: String,
    pub medium: String,
    pub mxid: String,
    pub room_id: String,
    pub sender: String,
    pub signed: SignedThirdPartyInvite,
}

impl ThirdPartyInvite {
    pub fn new(
        address: String,
        medium: String,
        mxid: String,
        room_id: String,
        sender: String,
        signed: SignedThirdPartyInvite,
    ) -> Self {
        Self { address, medium, mxid, room_id, sender, signed }
    }
}
