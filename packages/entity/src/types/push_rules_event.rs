use crate::types::Ruleset;
use serde::{Deserialize, Serialize};

/// PushRulesEvent
/// Source: spec/client/05_advanced_md:1978
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRulesEvent {
    pub global: Ruleset,
}

impl PushRulesEvent {
    pub fn new(global: Ruleset) -> Self {
        Self { global }
    }
}
