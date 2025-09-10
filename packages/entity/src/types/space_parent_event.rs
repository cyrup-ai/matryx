use serde::{Deserialize, Serialize};

/// SpaceParentEvent
/// Source: spec/client/07_relationship_md:182-185
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceParentEvent {
    pub canonical: Option<bool>,
    pub via: Vec<String>,
}

impl SpaceParentEvent {
    pub fn new(canonical: Option<bool>, via: Vec<String>) -> Self {
        Self { canonical, via }
    }
}
