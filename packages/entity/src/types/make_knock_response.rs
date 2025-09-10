use crate::types::KnockEventTemplate;
use serde::{Deserialize, Serialize};

/// MakeKnockResponse
/// Source: spec/server/12-room-md:65-70
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeKnockResponse {
    pub event: KnockEventTemplate,
    pub room_version: String,
}

impl MakeKnockResponse {
    pub fn new(event: KnockEventTemplate, room_version: String) -> Self {
        Self { event, room_version }
    }
}
