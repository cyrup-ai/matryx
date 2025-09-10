use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Query request for Matrix key queries
/// Represents device key query requests in the Matrix protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub device_keys: HashMap<String, Vec<String>>,
    pub timeout: Option<u64>,
    pub token: Option<String>,
}

impl QueryRequest {
    pub fn new(device_keys: HashMap<String, Vec<String>>) -> Self {
        Self { device_keys, timeout: None, token: None }
    }

    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }
}
