use crate::types::SignatureMap;
use serde::{Deserialize, Serialize};

/// BackupAuthData
/// Source: spec/client/04_security_md:1895-1897
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupAuthData {
    pub public_key: String,
    pub signatures: SignatureMap,
}

impl BackupAuthData {
    pub fn new(public_key: String, signatures: SignatureMap) -> Self {
        Self { public_key, signatures }
    }
}
