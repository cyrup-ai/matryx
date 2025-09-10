use crate::types::DirectToDeviceContent;
use serde::{Deserialize, Serialize};

/// DirectToDeviceEDU
/// Source: spec/server/17-device-md:311-327
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectToDeviceEDU {
    pub content: DirectToDeviceContent,
    pub edu_type: String,
}

impl DirectToDeviceEDU {
    pub fn new(content: DirectToDeviceContent, edu_type: String) -> Self {
        Self { content, edu_type }
    }
}
