use crate::types::SessionData;
use serde::{Deserialize, Serialize};

/// KeyBackupData
/// Source: spec/client/04_security_md:1947-1951
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBackupData {
    pub first_message_index: i64,
    pub forwarded_count: i64,
    pub is_verified: bool,
    pub session_data: SessionData,
}

impl KeyBackupData {
    pub fn new(
        first_message_index: i64,
        forwarded_count: i64,
        is_verified: bool,
        session_data: SessionData,
    ) -> Self {
        Self {
            first_message_index,
            forwarded_count,
            is_verified,
            session_data,
        }
    }
}
