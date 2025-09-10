use serde::{Deserialize, Serialize};

/// PushRuleEnabledGetResponse
/// Source: spec/client/05_advanced_md:1892
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRuleEnabledGetResponse {
    pub enabled: bool,
}

impl PushRuleEnabledGetResponse {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}
