use crate::types::ServerKeysResponse;
use serde::{Deserialize, Serialize};

/// Key query response (server)
/// Source: spec/server/03-server-md:104-131
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyQueryResponseServer {
    pub server_keys: Vec<ServerKeysResponse>,
}

impl KeyQueryResponseServer {
    pub fn new(server_keys: Vec<ServerKeysResponse>) -> Self {
        Self { server_keys }
    }
}
