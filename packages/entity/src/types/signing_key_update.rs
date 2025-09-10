use crate::types::CrossSigningKey;
use serde::{Deserialize, Serialize};

/// Signing key update
/// Source: spec/server/27-end-to-end-md:251-253
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKeyUpdate {
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub user_id: String,
}

impl SigningKeyUpdate {
    pub fn new(
        master_key: Option<CrossSigningKey>,
        self_signing_key: Option<CrossSigningKey>,
        user_id: String,
    ) -> Self {
        Self { master_key, self_signing_key, user_id }
    }
}
