use crate::types::EventContent;
use serde::{Deserialize, Serialize};

/// GlobalAccountData
/// Source: spec/client/06_user_md:267-306
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAccountData {
    #[serde(rename = "type")]
    pub event_type: String,
    pub content: EventContent,
}

impl GlobalAccountData {
    pub fn new(event_type: String, content: EventContent) -> Self {
        Self { event_type, content }
    }
}
