use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One-time key object for key exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKeyObject {
    /// Key algorithm and key data
    #[serde(flatten)]
    pub keys: HashMap<String, String>,

    /// Key signatures
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
}

impl OneTimeKeyObject {
    pub fn new() -> Self {
        Self { keys: HashMap::new(), signatures: None }
    }
}

impl Default for OneTimeKeyObject {
    fn default() -> Self {
        Self::new()
    }
}
