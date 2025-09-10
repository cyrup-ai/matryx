use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OneTimeKeyClaimRequest
/// Source: spec/server/17-device-md:84-91
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKeyClaimRequest {
    pub one_time_keys: HashMap<String, HashMap<String, String>>,
}

impl OneTimeKeyClaimRequest {
    pub fn new(one_time_keys: HashMap<String, HashMap<String, String>>) -> Self {
        Self { one_time_keys }
    }
}
