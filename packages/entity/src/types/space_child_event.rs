use serde::{Deserialize, Serialize};

/// SpaceChildEvent
/// Source: spec/client/07_relationship_md:102-106
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceChildEvent {
    pub order: Option<String>,
    pub suggested: Option<bool>,
    pub via: Vec<String>,
}

impl SpaceChildEvent {
    pub fn new(order: Option<String>, suggested: Option<bool>, via: Vec<String>) -> Self {
        Self { order, suggested, via }
    }
}
