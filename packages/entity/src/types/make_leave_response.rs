use crate::types::LeaveEventTemplate;
use serde::{Deserialize, Serialize};

/// MakeLeaveResponse
/// Source: spec/server/10-room-md:34-43
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeLeaveResponse {
    pub event: LeaveEventTemplate,
    pub room_version: String,
}

impl MakeLeaveResponse {
    pub fn new(event: LeaveEventTemplate, room_version: String) -> Self {
        Self { event, room_version }
    }
}
