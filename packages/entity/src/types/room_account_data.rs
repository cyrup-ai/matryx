use crate::types::AccountDataContent;
use serde::{Deserialize, Serialize};

/// RoomAccountData
/// Source: spec/client/06_user_md:418-495
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomAccountData {
    #[serde(rename = "type")]
    pub event_type: String,
    pub room_id: String,
    pub content: AccountDataContent,
}

impl RoomAccountData {
    pub fn new(event_type: String, room_id: String, content: AccountDataContent) -> Self {
        Self { event_type, room_id, content }
    }
}
