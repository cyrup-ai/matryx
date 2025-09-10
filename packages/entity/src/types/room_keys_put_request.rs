use crate::types::RoomKeyBackup;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RoomKeysPutRequest
/// Source: spec/client/04_security_md:2020
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysPutRequest {
    pub rooms: HashMap<String, RoomKeyBackup>,
}

impl RoomKeysPutRequest {
    pub fn new(rooms: HashMap<String, RoomKeyBackup>) -> Self {
        Self { rooms }
    }
}
