use crate::types::DeviceListUpdate;
use serde::{Deserialize, Serialize};

/// DeviceListUpdateEDU
/// Source: spec/server/07-md:128-132
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListUpdateEDU {
    pub content: DeviceListUpdate,
    pub edu_type: String,
}

impl DeviceListUpdateEDU {
    pub fn new(content: DeviceListUpdate, edu_type: String) -> Self {
        Self { content, edu_type }
    }
}
