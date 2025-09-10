use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Key claim request
/// Source: spec/server/27-end-to-end-md:39
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyClaimRequest {
    pub one_time_keys: HashMap<String, HashMap<String, String>>,
}

impl KeyClaimRequest {
    pub fn new(one_time_keys: HashMap<String, HashMap<String, String>>) -> Self {
        Self { one_time_keys }
    }
}
