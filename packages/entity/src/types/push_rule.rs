use crate::types::{PushAction, PushCondition};
use serde::{Deserialize, Serialize};

/// Push notification rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRule {
    /// Rule ID
    pub rule_id: String,

    /// Rule priority/default status
    pub default: bool,

    /// Whether the rule is enabled
    pub enabled: bool,

    /// Rule pattern (for content rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// Rule conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<PushCondition>>,

    /// Rule actions
    pub actions: Vec<PushAction>,
}

impl PushRule {
    pub fn new(rule_id: String, default: bool, enabled: bool, actions: Vec<PushAction>) -> Self {
        Self {
            rule_id,
            default,
            enabled,
            pattern: None,
            conditions: None,
            actions,
        }
    }
}
