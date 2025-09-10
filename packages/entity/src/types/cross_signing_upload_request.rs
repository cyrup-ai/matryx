use crate::types::{AuthenticationData, CrossSigningKey};
use serde::{Deserialize, Serialize};

/// CrossSigningUploadRequest
/// Source: spec/client/04_security_md:1506-1510
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningUploadRequest {
    pub auth: AuthenticationData,
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub user_signing_key: Option<CrossSigningKey>,
}

impl CrossSigningUploadRequest {
    pub fn new(
        auth: AuthenticationData,
        master_key: Option<CrossSigningKey>,
        self_signing_key: Option<CrossSigningKey>,
        user_signing_key: Option<CrossSigningKey>,
    ) -> Self {
        Self {
            auth,
            master_key,
            self_signing_key,
            user_signing_key,
        }
    }
}
