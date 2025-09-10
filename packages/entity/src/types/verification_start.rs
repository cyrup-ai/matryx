use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// VerificationStart
/// Source: spec/client/04_security_md:828-834
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationStart {
    pub from_device: String,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub method: String,
    pub next_method: Option<String>,
    pub transaction_id: Option<String>,
}

impl VerificationStart {
    pub fn new(
        from_device: String,
        m_relates_to: Option<VerificationRelatesTo>,
        method: String,
        next_method: Option<String>,
        transaction_id: Option<String>,
    ) -> Self {
        Self {
            from_device,
            m_relates_to,
            method,
            next_method,
            transaction_id,
        }
    }
}
