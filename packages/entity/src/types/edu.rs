use crate::types::EphemeralEvent;
use serde::{Deserialize, Serialize};

/// Ephemeral Data Unit (EDU) - Matrix federation ephemeral event structure
///
/// EDUs represent temporary events that are not stored in room history, such as
/// typing notifications, read receipts, and presence updates. They are exchanged
/// between homeservers during federation but are not persisted.
///
/// **Matrix Specification:** Server-Server API Ephemeral Events
/// **Source:** spec/server/01-md:16-17
/// **Purpose:** Temporary events (typing, presence, read receipts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EDU {
    /// The ephemeral event data
    pub ephemeral_event: EphemeralEvent,
    /// Flag indicating this event should not be persisted
    pub non_persistent: bool,
}

impl EDU {
    pub fn new(ephemeral_event: EphemeralEvent, non_persistent: bool) -> Self {
        Self { ephemeral_event, non_persistent }
    }
}
