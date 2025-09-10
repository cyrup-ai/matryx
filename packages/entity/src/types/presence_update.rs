use crate::types::UserPresenceUpdate;
use serde::{Deserialize, Serialize};

/// PresenceUpdate
/// Source: spec/server/07-md:52-56
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceUpdate {
    pub push: Vec<UserPresenceUpdate>,
}

impl PresenceUpdate {
    pub fn new(push: Vec<UserPresenceUpdate>) -> Self {
        Self { push }
    }
}
