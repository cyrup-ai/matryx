use crate::types::RoomKeyBackup;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RoomKeysGetResponse
/// Source: spec/client/04_security_md:1943
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysGetResponse {
    pub rooms: HashMap<String, RoomKeyBackup>,
}

impl RoomKeysGetResponse {
    pub fn new(rooms: HashMap<String, RoomKeyBackup>) -> Self {
        Self { rooms }
    }
}
