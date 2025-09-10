use crate::types::{CrossSigningKey, DeviceKeys};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Key query response
/// Source: spec/server/27-end-to-end-md:158-160
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyQueryResponse {
    pub device_keys: HashMap<String, HashMap<String, DeviceKeys>>,
    pub master_keys: Option<HashMap<String, CrossSigningKey>>,
    pub self_signing_keys: Option<HashMap<String, CrossSigningKey>>,
}

impl KeyQueryResponse {
    pub fn new(
        device_keys: HashMap<String, HashMap<String, DeviceKeys>>,
        master_keys: Option<HashMap<String, CrossSigningKey>>,
        self_signing_keys: Option<HashMap<String, CrossSigningKey>>,
    ) -> Self {
        Self { device_keys, master_keys, self_signing_keys }
    }
}
