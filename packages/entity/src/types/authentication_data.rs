use serde::{Deserialize, Serialize};

/// Authentication data
/// Source: spec/client/04_security_md:148-152
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationData {
    pub session: Option<String>,
    #[serde(rename = "type")]
    pub auth_type: Option<String>,
}

impl AuthenticationData {
    pub fn new(session: Option<String>, auth_type: Option<String>) -> Self {
        Self { session, auth_type }
    }
}
