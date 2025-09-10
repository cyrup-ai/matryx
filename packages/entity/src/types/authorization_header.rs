use serde::{Deserialize, Serialize};

/// AuthorizationHeader
/// Source: spec/server/04-md:52-59
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationHeader {
    pub origin: String,
    pub destination: Option<String>,
    pub key: String,
    pub signature: String,
}

impl AuthorizationHeader {
    pub fn new(
        origin: String,
        destination: Option<String>,
        key: String,
        signature: String,
    ) -> Self {
        Self { origin, destination, key, signature }
    }
}
