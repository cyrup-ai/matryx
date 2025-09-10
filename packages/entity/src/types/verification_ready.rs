use crate::types::VerificationRelatesTo;
use serde::{Deserialize, Serialize};

/// VerificationReady
/// Source: spec/client/04_security_md:806-810
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReady {
    pub from_device: String,
    pub m_relates_to: Option<VerificationRelatesTo>,
    pub methods: Vec<String>,
    pub transaction_id: Option<String>,
}

impl VerificationReady {
    pub fn new(
        from_device: String,
        m_relates_to: Option<VerificationRelatesTo>,
        methods: Vec<String>,
        transaction_id: Option<String>,
    ) -> Self {
        Self { from_device, m_relates_to, methods, transaction_id }
    }
}
