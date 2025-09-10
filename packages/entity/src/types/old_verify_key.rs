use serde::{Deserialize, Serialize};

/// Old verify key
/// Source: spec/server/03-server-md:64-66
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OldVerifyKey {
    pub key: String,
    pub expired_ts: i64,
}

impl OldVerifyKey {
    pub fn new(key: String, expired_ts: i64) -> Self {
        Self { key, expired_ts }
    }
}
