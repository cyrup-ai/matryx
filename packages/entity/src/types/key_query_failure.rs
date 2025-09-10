use serde::{Deserialize, Serialize};

/// Key query failure information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyQueryFailure {
    /// Error code for the failure
    pub errcode: String,

    /// Human-readable error message
    pub error: String,
}

impl KeyQueryFailure {
    pub fn new(errcode: String, error: String) -> Self {
        Self { errcode, error }
    }
}
