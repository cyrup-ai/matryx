use serde::{Deserialize, Serialize};

/// Knock room response
/// Source: spec/client/02_rooms_md:901
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockRoomResponse {
    pub room_id: String,
}

impl KnockRoomResponse {
    pub fn new(room_id: String) -> Self {
        Self { room_id }
    }
}
