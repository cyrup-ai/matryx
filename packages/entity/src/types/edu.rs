use crate::types::EphemeralEvent;
use serde::{Deserialize, Serialize};

/// EDU (Ephemeral Data Unit)
/// Source: spec/server/01-md:16-17
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EDU {
    pub ephemeral_event: EphemeralEvent,
    pub non_persistent: bool,
}

impl EDU {
    pub fn new(ephemeral_event: EphemeralEvent, non_persistent: bool) -> Self {
        Self { ephemeral_event, non_persistent }
    }
}
