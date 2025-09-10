use serde::{Deserialize, Serialize};

/// OpenID user info response
/// Source: spec/server/26-md:52
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenIdUserInfoResponse {
    pub sub: String,
}

impl OpenIdUserInfoResponse {
    pub fn new(sub: String) -> Self {
        Self { sub }
    }
}
