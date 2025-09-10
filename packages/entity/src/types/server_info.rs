use crate::types::ServerDetails;
use serde::{Deserialize, Serialize};

/// Server info
/// Source: spec/server/01-md:72-78
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub server: ServerDetails,
}

impl ServerInfo {
    pub fn new(server: ServerDetails) -> Self {
        Self { server }
    }
}
