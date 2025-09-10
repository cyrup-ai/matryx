use serde::{Deserialize, Serialize};

/// Encrypted data for Matrix key backup
/// Represents encrypted session data in key backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub ciphertext: String,
    pub mac: String,
    pub ephemeral: String,
}

impl EncryptedData {
    pub fn new(ciphertext: String, mac: String, ephemeral: String) -> Self {
        Self { ciphertext, mac, ephemeral }
    }
}
