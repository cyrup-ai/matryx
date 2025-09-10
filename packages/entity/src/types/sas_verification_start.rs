use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// SASVerificationStart
/// Source: spec/client/04_security_md:1157-1165
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SASVerificationStart {
    pub from_device: String,
    pub hashes: Vec<String>,
    pub key_agreement_protocols: Vec<String>,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub message_authentication_codes: Vec<String>,
    pub method: String,
    pub short_authentication_string: Vec<String>,
    pub transaction_id: Option<String>,
}

impl SASVerificationStart {
    pub fn new(
        from_device: String,
        hashes: Vec<String>,
        key_agreement_protocols: Vec<String>,
        m_relates_to: Option<VerificationRelatesTo>,
        message_authentication_codes: Vec<String>,
        method: String,
        short_authentication_string: Vec<String>,
        transaction_id: Option<String>,
    ) -> Self {
        Self {
            from_device,
            hashes,
            key_agreement_protocols,
            m_relates_to,
            message_authentication_codes,
            method,
            short_authentication_string,
            transaction_id,
        }
    }
}
