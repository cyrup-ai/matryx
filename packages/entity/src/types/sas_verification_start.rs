use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// Parameters for creating a SAS verification start
#[derive(Debug, Clone)]
pub struct SasVerificationParams {
    pub from_device: String,
    pub hashes: Vec<String>,
    pub key_agreement_protocols: Vec<String>,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub message_authentication_codes: Vec<String>,
    pub method: String,
    pub short_authentication_string: Vec<String>,
    pub transaction_id: Option<String>,
}

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
    pub fn new(params: SasVerificationParams) -> Self {
        Self {
            from_device: params.from_device,
            hashes: params.hashes,
            key_agreement_protocols: params.key_agreement_protocols,
            m_relates_to: params.m_relates_to,
            message_authentication_codes: params.message_authentication_codes,
            method: params.method,
            short_authentication_string: params.short_authentication_string,
            transaction_id: params.transaction_id,
        }
    }
}
