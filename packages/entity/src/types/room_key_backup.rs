use crate::types::KeyBackupData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Room key backup data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeyBackup {
    /// Sessions keyed by session ID
    pub sessions: HashMap<String, KeyBackupData>,
}

impl RoomKeyBackup {
    pub fn new() -> Self {
        Self { sessions: HashMap::new() }
    }

    pub fn add_session(&mut self, session_id: String, data: KeyBackupData) {
        self.sessions.insert(session_id, data);
    }
}
