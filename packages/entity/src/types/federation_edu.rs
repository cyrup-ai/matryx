use crate::types::EphemeralEvent;
use serde::{Deserialize, Serialize};

/// FederationEDU
/// Source: spec/server/05-md:44-46
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationEDU {
    pub edu_type: String,
    pub content: EphemeralEvent,
}

impl FederationEDU {
    pub fn new(edu_type: String, content: EphemeralEvent) -> Self {
        Self { edu_type, content }
    }
}
