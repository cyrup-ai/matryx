use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// VerificationDone
/// Source: spec/client/04_security_md:861-863
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDone {
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub transaction_id: Option<String>,
}

impl VerificationDone {
    pub fn new(
        m_relates_to: Option<VerificationRelatesTo>,
        transaction_id: Option<String>,
    ) -> Self {
        Self { m_relates_to, transaction_id }
    }
}
