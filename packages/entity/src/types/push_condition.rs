use crate::types::PushConditionValue;
use serde::{Deserialize, Serialize};

/// PushCondition
/// Source: spec/client/05_advanced_md:1034-1040
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushCondition {
    pub is: Option<String>,
    pub key: Option<String>,
    pub kind: String,
    pub pattern: Option<String>,
    pub value: Option<PushConditionValue>,
}

impl PushCondition {
    pub fn new(
        is: Option<String>,
        key: Option<String>,
        kind: String,
        pattern: Option<String>,
        value: Option<PushConditionValue>,
    ) -> Self {
        Self { is, key, kind, pattern, value }
    }
}
