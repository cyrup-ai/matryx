use crate::types::Device;
use serde::{Deserialize, Serialize};

/// Devices list response
/// Source: spec/client/04_security_md:198
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicesListResponse {
    pub devices: Vec<Device>,
}

impl DevicesListResponse {
    pub fn new(devices: Vec<Device>) -> Self {
        Self { devices }
    }
}
