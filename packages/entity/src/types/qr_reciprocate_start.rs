use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// QRReciprocateStart
/// Source: spec/client/04_security_md:1818-1822
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRReciprocateStart {
    pub from_device: String,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub method: String,
    pub secret: String,
    pub transaction_id: Option<String>,
}

impl QRReciprocateStart {
    pub fn new(
        from_device: String,
        m_relates_to: Option<VerificationRelatesTo>,
        method: String,
        secret: String,
        transaction_id: Option<String>,
    ) -> Self {
        Self {
            from_device,
            m_relates_to,
            method,
            secret,
            transaction_id,
        }
    }
}
