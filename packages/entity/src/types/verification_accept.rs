use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// VerificationAccept
/// Source: spec/client/04_security_md:1188-1196
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationAccept {
    pub commitment: String,
    pub hash: String,
    pub key_agreement_protocol: String,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub message_authentication_code: String,
    pub short_authentication_string: Vec<String>,
    pub transaction_id: Option<String>,
}

impl VerificationAccept {
    pub fn new(
        commitment: String,
        hash: String,
        key_agreement_protocol: String,
        m_relates_to: Option<VerificationRelatesTo>,
        message_authentication_code: String,
        short_authentication_string: Vec<String>,
        transaction_id: Option<String>,
    ) -> Self {
        Self {
            commitment,
            hash,
            key_agreement_protocol,
            m_relates_to,
            message_authentication_code,
            short_authentication_string,
            transaction_id,
        }
    }
}
