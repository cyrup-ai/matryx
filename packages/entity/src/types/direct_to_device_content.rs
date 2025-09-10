use crate::types::EncryptedContent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// DirectToDeviceContent
/// Source: spec/server/17-device-md:313-327
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectToDeviceContent {
    pub sender: String,
    pub message_id: String,
    pub messages: HashMap<String, HashMap<String, EncryptedContent>>,
}

impl DirectToDeviceContent {
    pub fn new(
        sender: String,
        message_id: String,
        messages: HashMap<String, HashMap<String, EncryptedContent>>,
    ) -> Self {
        Self { sender, message_id, messages }
    }
}
