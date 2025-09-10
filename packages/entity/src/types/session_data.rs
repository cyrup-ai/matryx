use crate::types::EncryptedData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Session data for key backup
/// Represents encrypted session data stored in key backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Encrypted session data
    #[serde(flatten)]
    pub encrypted_data: HashMap<String, EncryptedData>,
}

impl SessionData {
    pub fn new() -> Self {
        Self { encrypted_data: HashMap::new() }
    }

    pub fn with_field(mut self, key: String, value: EncryptedData) -> Self {
        self.encrypted_data.insert(key, value);
        self
    }
}

impl Default for SessionData {
    fn default() -> Self {
        Self::new()
    }
}
