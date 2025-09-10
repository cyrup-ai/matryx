use crate::types::PublicKeys;
use serde::{Deserialize, Serialize};

/// ThirdPartyInviteEvent
/// Source: spec/client/05_advanced_md:2065-2069
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyInviteEvent {
    pub display_name: String,
    pub key_validity_url: String,
    pub public_key: String,
    pub public_keys: Vec<PublicKeys>,
}

impl ThirdPartyInviteEvent {
    pub fn new(
        display_name: String,
        key_validity_url: String,
        public_key: String,
        public_keys: Vec<PublicKeys>,
    ) -> Self {
        Self {
            display_name,
            key_validity_url,
            public_key,
            public_keys,
        }
    }
}
