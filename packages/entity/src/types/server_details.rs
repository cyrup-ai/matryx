use serde::{Deserialize, Serialize};

/// Server details
/// Source: spec/server/01-md:74-77
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDetails {
    pub name: String,
    pub version: String,
}

impl ServerDetails {
    pub fn new(name: String, version: String) -> Self {
        Self { name, version }
    }
}
