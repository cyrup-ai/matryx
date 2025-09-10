use crate::types::KeyBackupData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RoomKeysByRoomPutRequest
/// Source: spec/client/04_security_md:2394
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysByRoomPutRequest {
    pub sessions: HashMap<String, KeyBackupData>,
}

impl RoomKeysByRoomPutRequest {
    pub fn new(sessions: HashMap<String, KeyBackupData>) -> Self {
        Self { sessions }
    }
}
