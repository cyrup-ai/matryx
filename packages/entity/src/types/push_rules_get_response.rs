use crate::types::Ruleset;
use serde::{Deserialize, Serialize};

/// PushRulesGetResponse
/// Source: spec/client/05_advanced_md:1018
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRulesGetResponse {
    pub global: Ruleset,
}

impl PushRulesGetResponse {
    pub fn new(global: Ruleset) -> Self {
        Self { global }
    }
}
