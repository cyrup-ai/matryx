use serde::{Deserialize, Serialize};

/// Unsigned device info
/// Source: spec/server/27-end-to-end-md:173
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsignedDeviceInfo {
    pub device_display_name: Option<String>,
}

impl UnsignedDeviceInfo {
    pub fn new(device_display_name: Option<String>) -> Self {
        Self { device_display_name }
    }
}
