use crate::types::PushRule;
use serde::{Deserialize, Serialize};

/// Ruleset
/// Source: spec/client/05_advanced_md:1020-1024
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ruleset {
    pub content: Vec<PushRule>,
    #[serde(rename = "override")]
    pub override_rules: Vec<PushRule>,
    pub room: Vec<PushRule>,
    pub sender: Vec<PushRule>,
    pub underride: Vec<PushRule>,
}

impl Ruleset {
    pub fn new(
        content: Vec<PushRule>,
        override_rules: Vec<PushRule>,
        room: Vec<PushRule>,
        sender: Vec<PushRule>,
        underride: Vec<PushRule>,
    ) -> Self {
        Self { content, override_rules, room, sender, underride }
    }
}
