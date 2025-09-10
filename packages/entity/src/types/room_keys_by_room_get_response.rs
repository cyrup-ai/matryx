use crate::types::KeyBackupData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RoomKeysByRoomGetResponse
/// Source: spec/client/04_security_md:2307
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysByRoomGetResponse {
    pub sessions: HashMap<String, KeyBackupData>,
}

impl RoomKeysByRoomGetResponse {
    pub fn new(sessions: HashMap<String, KeyBackupData>) -> Self {
        Self { sessions }
    }
}
