use crate::types::SignatureError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SignaturesUploadResponse
/// Source: spec/client/04_security_md:1668
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignaturesUploadResponse {
    pub failures: HashMap<String, HashMap<String, SignatureError>>,
}

impl SignaturesUploadResponse {
    pub fn new(failures: HashMap<String, HashMap<String, SignatureError>>) -> Self {
        Self { failures }
    }
}
