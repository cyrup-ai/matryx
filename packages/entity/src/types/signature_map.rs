use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signature map for Matrix cryptographic signatures
/// Represents signatures in the Matrix protocol format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureMap {
    #[serde(flatten)]
    pub signatures: HashMap<String, String>,
}

impl SignatureMap {
    pub fn new() -> Self {
        Self { signatures: HashMap::new() }
    }

    pub fn add_signature(mut self, key_id: String, signature: String) -> Self {
        self.signatures.insert(key_id, signature);
        self
    }
}

impl Default for SignatureMap {
    fn default() -> Self {
        Self::new()
    }
}
