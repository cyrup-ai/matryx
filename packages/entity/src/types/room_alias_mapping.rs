use serde::{Deserialize, Serialize};

/// Room alias mapping
/// Source: spec/client/02_rooms_md:217
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomAliasMapping {
    pub room_id: String,
}

impl RoomAliasMapping {
    pub fn new(room_id: String) -> Self {
        Self { room_id }
    }
}
