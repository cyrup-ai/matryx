use crate::types::OneTimeKeyObject;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OneTimeKeyClaimResponse
/// Source: spec/server/17-device-md:93-111
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKeyClaimResponse {
    pub one_time_keys: HashMap<String, HashMap<String, HashMap<String, OneTimeKeyObject>>>,
}

impl OneTimeKeyClaimResponse {
    pub fn new(
        one_time_keys: HashMap<String, HashMap<String, HashMap<String, OneTimeKeyObject>>>,
    ) -> Self {
        Self { one_time_keys }
    }
}
