use serde::{Deserialize, Serialize};

/// OpenID error response
/// Source: spec/server/26-md:66-67
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenIdErrorResponse {
    pub errcode: String,
    pub error: Option<String>,
}

impl OpenIdErrorResponse {
    pub fn new(errcode: String, error: Option<String>) -> Self {
        Self { errcode, error }
    }
}
