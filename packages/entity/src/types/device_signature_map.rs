use crate::types::SignatureMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Device signature map for Matrix device signatures
/// Represents signatures for device keys: user_id -> device_id -> signature_object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSignatureMap {
    /// Map of user IDs to device signatures
    #[serde(flatten)]
    pub signatures: HashMap<String, HashMap<String, SignatureMap>>,
}

impl DeviceSignatureMap {
    pub fn new() -> Self {
        Self { signatures: HashMap::new() }
    }

    pub fn add_device_signature(
        &mut self,
        user_id: String,
        device_id: String,
        signature: SignatureMap,
    ) {
        self.signatures.entry(user_id).or_default().insert(device_id, signature);
    }
}

impl Default for DeviceSignatureMap {
    fn default() -> Self {
        Self::new()
    }
}
