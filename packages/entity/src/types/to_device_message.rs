use crate::types::EventContent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ToDeviceMessage
/// Source: spec/server/18-send-to-md:18-27
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToDeviceMessage {
    pub message_id: String,
    pub messages: HashMap<String, HashMap<String, EventContent>>,
    pub sender: String,
    #[serde(rename = "type")]
    pub event_type: String,
}

impl ToDeviceMessage {
    pub fn new(
        message_id: String,
        messages: HashMap<String, HashMap<String, EventContent>>,
        sender: String,
        event_type: String,
    ) -> Self {
        Self { message_id, messages, sender, event_type }
    }
}
