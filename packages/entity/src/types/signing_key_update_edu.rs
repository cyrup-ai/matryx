use crate::types::SigningKeyUpdate;
use serde::{Deserialize, Serialize};

/// Signing key update EDU
/// Source: spec/server/27-end-to-end-md:246-247
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKeyUpdateEDU {
    pub content: SigningKeyUpdate,
    pub edu_type: String,
}

impl SigningKeyUpdateEDU {
    pub fn new(content: SigningKeyUpdate, edu_type: String) -> Self {
        Self { content, edu_type }
    }
}
