use serde::{Deserialize, Serialize};

/// Join room response
/// Source: spec/client/02_rooms_md:661
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRoomResponse {
    pub room_id: String,
}

impl JoinRoomResponse {
    pub fn new(room_id: String) -> Self {
        Self { room_id }
    }
}
