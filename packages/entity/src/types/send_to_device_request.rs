use crate::types::EventContent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Send to device request
/// Source: spec/client/04_security_md:54
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendToDeviceRequest {
    pub messages: HashMap<String, HashMap<String, EventContent>>,
}

impl SendToDeviceRequest {
    pub fn new(messages: HashMap<String, HashMap<String, EventContent>>) -> Self {
        Self { messages }
    }
}
