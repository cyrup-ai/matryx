use crate::types::JWK;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Encrypted file
/// Source: spec/client/04_security_md:612-617
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedFile {
    pub url: String,
    pub key: JWK,
    pub iv: String,
    pub hashes: HashMap<String, String>,
    pub v: String,
}

impl EncryptedFile {
    pub fn new(
        url: String,
        key: JWK,
        iv: String,
        hashes: HashMap<String, String>,
        v: String,
    ) -> Self {
        Self { url, key, iv, hashes, v }
    }
}
