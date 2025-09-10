use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cryptographic device information for end-to-end encryption
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CryptoDevice {
    /// Device ID
    pub device_id: String,

    /// User ID that owns this device
    pub user_id: String,

    /// Device display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Device keys for encryption
    pub keys: HashMap<String, String>,

    /// Device algorithms supported
    pub algorithms: Vec<String>,

    /// Device signatures
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,

    /// Whether the device is verified
    pub verified: bool,

    /// Whether the device is blocked
    pub blocked: bool,

    /// One-time keys count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_time_keys_count: Option<HashMap<String, i64>>,

    /// Fallback keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_keys: Option<HashMap<String, serde_json::Value>>,
}

impl CryptoDevice {
    /// Create a new crypto device
    pub fn new(
        device_id: String,
        user_id: String,
        keys: HashMap<String, String>,
        algorithms: Vec<String>,
    ) -> Self {
        Self {
            device_id,
            user_id,
            display_name: None,
            keys,
            algorithms,
            signatures: None,
            verified: false,
            blocked: false,
            one_time_keys_count: None,
            fallback_keys: None,
        }
    }
}