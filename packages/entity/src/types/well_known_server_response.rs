use serde::{Deserialize, Serialize};

/// Well known server response
/// Source: spec/server/02-server-md:54-60
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WellKnownServerResponse {
    #[serde(rename = "m.server")]
    pub m_server: String,
}

impl WellKnownServerResponse {
    pub fn new(m_server: String) -> Self {
        Self { m_server }
    }
}
