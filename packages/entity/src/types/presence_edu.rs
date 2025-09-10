use crate::types::PresenceUpdate;
use serde::{Deserialize, Serialize};

/// PresenceEDU
/// Source: spec/server/07-md:46-50
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEDU {
    pub content: PresenceUpdate,
    pub edu_type: String,
}

impl PresenceEDU {
    pub fn new(content: PresenceUpdate, edu_type: String) -> Self {
        Self { content, edu_type }
    }
}
