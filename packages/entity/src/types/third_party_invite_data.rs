use crate::types::SignedThirdPartyInvite;
use serde::{Deserialize, Serialize};

/// ThirdPartyInviteData
/// Source: spec/server/11-room-md:494-498
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyInviteData {
    pub display_name: String,
    pub signed: SignedThirdPartyInvite,
}

impl ThirdPartyInviteData {
    pub fn new(display_name: String, signed: SignedThirdPartyInvite) -> Self {
        Self { display_name, signed }
    }
}
