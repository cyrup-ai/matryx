use serde::{Deserialize, Serialize};

/// PublicKeys
/// Source: spec/client/05_advanced_md:2069
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeys {
    pub key: String,
}

impl PublicKeys {
    pub fn new(key: String) -> Self {
        Self { key }
    }
}
