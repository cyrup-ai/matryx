use crate::types::KeyObject;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Key claim response
/// Source: spec/server/27-end-to-end-md:71
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyClaimResponse {
    pub one_time_keys: HashMap<String, HashMap<String, HashMap<String, KeyObject>>>,
}

impl KeyClaimResponse {
    pub fn new(
        one_time_keys: HashMap<String, HashMap<String, HashMap<String, KeyObject>>>,
    ) -> Self {
        Self { one_time_keys }
    }
}
