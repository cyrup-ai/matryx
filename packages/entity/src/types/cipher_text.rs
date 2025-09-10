use serde::{Deserialize, Serialize};

/// Cipher text for encrypted content
/// Represents encrypted message data in Matrix protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherText {
    pub body: String,
    #[serde(rename = "type")]
    pub message_type: u8,
}

impl CipherText {
    pub fn new(body: String, message_type: u8) -> Self {
        Self { body, message_type }
    }
}
