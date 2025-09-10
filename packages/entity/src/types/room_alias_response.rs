use serde::{Deserialize, Serialize};

/// Room alias response
/// Source: spec/client/02_rooms_md:165-167
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomAliasResponse {
    pub room_id: String,
    pub servers: Vec<String>,
}

impl RoomAliasResponse {
    pub fn new(room_id: String, servers: Vec<String>) -> Self {
        Self { room_id, servers }
    }
}
