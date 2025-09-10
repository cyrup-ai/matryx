use crate::types::ThirdPartyInviteData;
use serde::{Deserialize, Serialize};

/// ThirdPartyInviteEventContent
/// Source: spec/server/11-room-md:487-492
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyInviteEventContent {
    pub membership: String,
    pub third_party_invite: ThirdPartyInviteData,
}

impl ThirdPartyInviteEventContent {
    pub fn new(membership: String, third_party_invite: ThirdPartyInviteData) -> Self {
        Self { membership, third_party_invite }
    }
}
