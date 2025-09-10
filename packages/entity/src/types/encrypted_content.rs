use crate::types::CipherText;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Encrypted content for to-device messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedContent {
    pub algorithm: String,
    pub ciphertext: HashMap<String, CipherText>,
    pub sender_key: String,
}

impl EncryptedContent {
    pub fn new(
        algorithm: String,
        ciphertext: HashMap<String, CipherText>,
        sender_key: String,
    ) -> Self {
        Self { algorithm, ciphertext, sender_key }
    }
}
