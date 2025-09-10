use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SignedThirdPartyInvite
/// Source: spec/server/11-room-md:387-395
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedThirdPartyInvite {
    pub mxid: String,
    pub signatures: HashMap<String, HashMap<String, String>>,
    pub token: String,
}

impl SignedThirdPartyInvite {
    pub fn new(
        mxid: String,
        signatures: HashMap<String, HashMap<String, String>>,
        token: String,
    ) -> Self {
        Self { mxid, signatures, token }
    }
}
