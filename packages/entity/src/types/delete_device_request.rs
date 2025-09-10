use crate::types::AuthenticationData;
use serde::{Deserialize, Serialize};

/// Delete device request
/// Source: spec/client/04_security_md:363
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteDeviceRequest {
    pub auth: AuthenticationData,
}

impl DeleteDeviceRequest {
    pub fn new(auth: AuthenticationData) -> Self {
        Self { auth }
    }
}
