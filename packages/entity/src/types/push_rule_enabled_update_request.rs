use serde::{Deserialize, Serialize};

/// PushRuleEnabledUpdateRequest
/// Source: spec/client/05_advanced_md:1936
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRuleEnabledUpdateRequest {
    pub enabled: bool,
}

impl PushRuleEnabledUpdateRequest {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}
