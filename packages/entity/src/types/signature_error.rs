use serde::{Deserialize, Serialize};

/// Signature error for Matrix signature upload responses
/// Represents errors that occur during signature verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureError {
    pub errcode: String,
    pub error: String,
}

impl SignatureError {
    pub fn new(errcode: String, error: String) -> Self {
        Self { errcode, error }
    }

    pub fn invalid_signature() -> Self {
        Self {
            errcode: "M_INVALID_SIGNATURE".to_string(),
            error: "Invalid signature".to_string(),
        }
    }

    pub fn unknown_device() -> Self {
        Self {
            errcode: "M_UNKNOWN_DEVICE".to_string(),
            error: "Unknown device".to_string(),
        }
    }
}
