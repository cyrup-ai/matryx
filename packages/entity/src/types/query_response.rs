use crate::types::{CrossSigningKey, DeviceKey, KeyQueryFailure};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Query response for Matrix key queries
/// Represents device key query responses in the Matrix protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub device_keys: HashMap<String, HashMap<String, DeviceKey>>,
    pub failures: Option<HashMap<String, KeyQueryFailure>>,
    pub master_keys: Option<HashMap<String, CrossSigningKey>>,
    pub self_signing_keys: Option<HashMap<String, CrossSigningKey>>,
    pub user_signing_keys: Option<HashMap<String, CrossSigningKey>>,
}

impl QueryResponse {
    pub fn new(device_keys: HashMap<String, HashMap<String, DeviceKey>>) -> Self {
        Self {
            device_keys,
            failures: None,
            master_keys: None,
            self_signing_keys: None,
            user_signing_keys: None,
        }
    }
}
