use serde::{Deserialize, Serialize};

/// Joined rooms response
/// Source: spec/client/02_rooms_md:484
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinedRoomsResponse {
    pub joined_rooms: Vec<String>,
}

impl JoinedRoomsResponse {
    pub fn new(joined_rooms: Vec<String>) -> Self {
        Self { joined_rooms }
    }
}
