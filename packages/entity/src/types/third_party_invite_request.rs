use serde::{Deserialize, Serialize};

/// ThirdPartyInviteRequest
/// Source: spec/client/05_advanced_md:2120-2124
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyInviteRequest {
    pub address: String,
    pub id_access_token: String,
    pub id_server: String,
    pub medium: String,
}

impl ThirdPartyInviteRequest {
    pub fn new(
        address: String,
        id_access_token: String,
        id_server: String,
        medium: String,
    ) -> Self {
        Self { address, id_access_token, id_server, medium }
    }
}
