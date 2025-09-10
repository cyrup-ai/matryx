use crate::types::EventTemplate;
use serde::{Deserialize, Serialize};

/// MakeJoinResponse
/// Source: spec/server/09-room-md:96-105
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeJoinResponse {
    pub event: EventTemplate,
    pub room_version: String,
}

impl MakeJoinResponse {
    pub fn new(event: EventTemplate, room_version: String) -> Self {
        Self { event, room_version }
    }
}
