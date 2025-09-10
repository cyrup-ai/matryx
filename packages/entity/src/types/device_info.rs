use crate::types::DeviceKeys;
use serde::{Deserialize, Serialize};

/// DeviceInfo
/// Source: spec/server/17-device-md:36-52
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_display_name: Option<String>,
    pub device_id: String,
    pub keys: DeviceKeys,
}

impl DeviceInfo {
    pub fn new(device_display_name: Option<String>, device_id: String, keys: DeviceKeys) -> Self {
        Self { device_display_name, device_id, keys }
    }
}
