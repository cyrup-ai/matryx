use serde::{Deserialize, Serialize};

/// Knock room request
/// Source: spec/client/02_rooms_md:885
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockRoomRequest {
    pub reason: Option<String>,
}

impl KnockRoomRequest {
    pub fn new(reason: Option<String>) -> Self {
        Self { reason }
    }
}
