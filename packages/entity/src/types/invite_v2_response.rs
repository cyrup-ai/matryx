use crate::types::InviteEvent;
use serde::{Deserialize, Serialize};

/// InviteV2Response
/// Source: spec/server/11-room-md:280-285
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteV2Response {
    pub event: InviteEvent,
}

impl InviteV2Response {
    pub fn new(event: InviteEvent) -> Self {
        Self { event }
    }
}
