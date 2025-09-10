use serde::{Deserialize, Serialize};

/// Room aliases response
/// Source: spec/client/02_rooms_md:365
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomAliasesResponse {
    pub aliases: Vec<String>,
}

impl RoomAliasesResponse {
    pub fn new(aliases: Vec<String>) -> Self {
        Self { aliases }
    }
}
