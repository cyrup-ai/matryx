use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// VerificationMAC
/// Source: spec/client/04_security_md:1245-1249
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationMAC {
    pub keys: String,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub mac: HashMap<String, String>,
    pub transaction_id: Option<String>,
}

impl VerificationMAC {
    pub fn new(
        keys: String,
        m_relates_to: Option<VerificationRelatesTo>,
        mac: HashMap<String, String>,
        transaction_id: Option<String>,
    ) -> Self {
        Self { keys, m_relates_to, mac, transaction_id }
    }
}
