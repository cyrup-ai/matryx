use crate::types::{PushAction, PushCondition};
use serde::{Deserialize, Serialize};

/// PushRuleCreateUpdateRequest
/// Source: spec/client/05_advanced_md:1638-1641
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRuleCreateUpdateRequest {
    pub actions: Vec<PushAction>,
    pub conditions: Option<Vec<PushCondition>>,
    pub pattern: Option<String>,
}

impl PushRuleCreateUpdateRequest {
    pub fn new(
        actions: Vec<PushAction>,
        conditions: Option<Vec<PushCondition>>,
        pattern: Option<String>,
    ) -> Self {
        Self { actions, conditions, pattern }
    }
}
