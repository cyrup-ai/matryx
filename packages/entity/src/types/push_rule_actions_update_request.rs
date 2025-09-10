use crate::types::PushAction;
use serde::{Deserialize, Serialize};

/// PushRuleActionsUpdateRequest
/// Source: spec/client/05_advanced_md:1848
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRuleActionsUpdateRequest {
    pub actions: Vec<PushAction>,
}

impl PushRuleActionsUpdateRequest {
    pub fn new(actions: Vec<PushAction>) -> Self {
        Self { actions }
    }
}
