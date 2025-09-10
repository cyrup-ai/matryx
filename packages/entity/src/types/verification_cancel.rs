use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// VerificationCancel
/// Source: spec/client/04_security_md:878-882
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCancel {
    pub code: String,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub reason: String,
    pub transaction_id: Option<String>,
}

impl VerificationCancel {
    pub fn new(
        code: String,
        m_relates_to: Option<VerificationRelatesTo>,
        reason: String,
        transaction_id: Option<String>,
    ) -> Self {
        Self { code, m_relates_to, reason, transaction_id }
    }
}
