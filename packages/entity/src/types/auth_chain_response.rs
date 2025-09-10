use crate::types::PDU;
use serde::{Deserialize, Serialize};

/// AuthChainResponse
/// Source: spec/server/06-md:156-173
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthChainResponse {
    pub auth_chain: Vec<PDU>,
}

impl AuthChainResponse {
    pub fn new(auth_chain: Vec<PDU>) -> Self {
        Self { auth_chain }
    }
}
