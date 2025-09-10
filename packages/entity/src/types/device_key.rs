use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Device key information for Matrix end-to-end encryption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKey {
    /// The ID of the user the device belongs to
    pub user_id: String,

    /// The ID of the device these keys belong to
    pub device_id: String,

    /// The encryption algorithms supported by this device
    pub algorithms: Vec<String>,

    /// Public identity keys for the device
    pub keys: HashMap<String, String>,

    /// Signatures for the device key object
    pub signatures: HashMap<String, HashMap<String, String>>,

    /// Additional device information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<HashMap<String, serde_json::Value>>,
}

impl DeviceKey {
    pub fn new(
        user_id: String,
        device_id: String,
        algorithms: Vec<String>,
        keys: HashMap<String, String>,
        signatures: HashMap<String, HashMap<String, String>>,
    ) -> Self {
        Self {
            user_id,
            device_id,
            algorithms,
            keys,
            signatures,
            unsigned: None,
        }
    }
}
