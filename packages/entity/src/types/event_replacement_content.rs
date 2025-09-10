use crate::types::{EventContent, EventRelatesTo};
use serde::{Deserialize, Serialize};

/// EventReplacementContent
/// Source: spec/client/07_relationship_md:364-378
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventReplacementContent {
    #[serde(rename = "m.new_content")]
    pub new_content: EventContent,
    #[serde(rename = "m.relates_to")]
    pub relates_to: EventRelatesTo,
}

impl EventReplacementContent {
    pub fn new(new_content: EventContent, relates_to: EventRelatesTo) -> Self {
        Self { new_content, relates_to }
    }
}
