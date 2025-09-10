use serde::{Deserialize, Serialize};

/// EventRelatesTo
/// Source: spec/client/07_relationship_md:372-377
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRelatesTo {
    pub rel_type: String,
    pub event_id: String,
}

impl EventRelatesTo {
    pub fn new(rel_type: String, event_id: String) -> Self {
        Self { rel_type, event_id }
    }
}
