use serde::{Deserialize, Serialize};

/// Verify key
/// Source: spec/server/03-server-md:61-62
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKey {
    pub key: String,
}

impl VerifyKey {
    pub fn new(key: String) -> Self {
        Self { key }
    }
}
