use serde::{Deserialize, Serialize};

/// Update device request
/// Source: spec/client/04_security_md:324
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeviceRequest {
    pub display_name: Option<String>,
}

impl UpdateDeviceRequest {
    pub fn new(display_name: Option<String>) -> Self {
        Self { display_name }
    }
}
