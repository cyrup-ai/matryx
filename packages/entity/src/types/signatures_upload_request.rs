use crate::types::DeviceSignatureMap;
use serde::{Deserialize, Serialize};

/// SignaturesUploadRequest
/// Source: spec/client/04_security_md:1547
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignaturesUploadRequest {
    pub signatures: DeviceSignatureMap,
}

impl SignaturesUploadRequest {
    pub fn new(signatures: DeviceSignatureMap) -> Self {
        Self { signatures }
    }
}
