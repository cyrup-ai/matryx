use crate::types::ThirdPartyInvite;
use serde::{Deserialize, Serialize};

/// ThirdPartyBindRequest
/// Source: spec/server/11-room-md:365-375
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyBindRequest {
    pub address: String,
    pub invites: Vec<ThirdPartyInvite>,
    pub medium: String,
    pub mxid: String,
}

impl ThirdPartyBindRequest {
    pub fn new(
        address: String,
        invites: Vec<ThirdPartyInvite>,
        medium: String,
        mxid: String,
    ) -> Self {
        Self { address, invites, medium, mxid }
    }
}
