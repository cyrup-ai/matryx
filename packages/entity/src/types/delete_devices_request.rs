use crate::types::AuthenticationData;
use serde::{Deserialize, Serialize};

/// Delete devices request
/// Source: spec/client/04_security_md:145-146
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteDevicesRequest {
    pub auth: AuthenticationData,
    pub devices: Vec<String>,
}

impl DeleteDevicesRequest {
    pub fn new(auth: AuthenticationData, devices: Vec<String>) -> Self {
        Self { auth, devices }
    }
}
