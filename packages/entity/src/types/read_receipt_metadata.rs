use serde::{Deserialize, Serialize};

/// Metadata for read receipts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadReceiptMetadata {
    /// Timestamp when the receipt was sent
    pub ts: i64,

    /// Thread ID if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

impl ReadReceiptMetadata {
    pub fn new(ts: i64) -> Self {
        Self { ts, thread_id: None }
    }
}
