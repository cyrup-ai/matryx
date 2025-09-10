use serde::{Deserialize, Serialize};

/// VerificationRequestToDevice
/// Source: spec/client/04_security_md:787-791
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRequestToDevice {
    pub from_device: String,
    pub methods: Vec<String>,
    pub timestamp: i64,
    pub transaction_id: String,
}

impl VerificationRequestToDevice {
    pub fn new(
        from_device: String,
        methods: Vec<String>,
        timestamp: i64,
        transaction_id: String,
    ) -> Self {
        Self { from_device, methods, timestamp, transaction_id }
    }
}
