use crate::types::UnsignedDeviceInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Device keys
/// Source: spec/server/27-end-to-end-md:164-169
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKeys {
    pub algorithms: Vec<String>,
    pub device_id: String,
    pub keys: HashMap<String, String>,
    pub signatures: HashMap<String, HashMap<String, String>>,
    pub unsigned: Option<UnsignedDeviceInfo>,
    pub user_id: String,
}

impl DeviceKeys {
    pub fn new(
        algorithms: Vec<String>,
        device_id: String,
        keys: HashMap<String, String>,
        signatures: HashMap<String, HashMap<String, String>>,
        unsigned: Option<UnsignedDeviceInfo>,
        user_id: String,
    ) -> Self {
        Self {
            algorithms,
            device_id,
            keys,
            signatures,
            unsigned,
            user_id,
        }
    }
}
