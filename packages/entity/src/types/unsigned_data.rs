use crate::types::StrippedStateEvent;
use serde::{Deserialize, Serialize};

/// Unsigned data for Matrix events
/// Represents unsigned data fields in Matrix events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsignedData {
    pub age: Option<u64>,
    pub prev_content: Option<StrippedStateEvent>,
    pub redacted_because: Option<StrippedStateEvent>,
    pub transaction_id: Option<String>,
}

impl UnsignedData {
    pub fn new() -> Self {
        Self {
            age: None,
            prev_content: None,
            redacted_because: None,
            transaction_id: None,
        }
    }
}

impl Default for UnsignedData {
    fn default() -> Self {
        Self::new()
    }
}
