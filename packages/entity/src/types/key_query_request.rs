use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Key query request
/// Source: spec/server/27-end-to-end-md:130
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyQueryRequest {
    pub device_keys: HashMap<String, Vec<String>>,
}

impl KeyQueryRequest {
    pub fn new(device_keys: HashMap<String, Vec<String>>) -> Self {
        Self { device_keys }
    }
}
