use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// VerificationKey
/// Source: spec/client/04_security_md:1223-1226
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationKey {
    pub key: String,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub transaction_id: Option<String>,
}

impl VerificationKey {
    pub fn new(
        key: String,
        m_relates_to: Option<VerificationRelatesTo>,
        transaction_id: Option<String>,
    ) -> Self {
        Self { key, m_relates_to, transaction_id }
    }
}
