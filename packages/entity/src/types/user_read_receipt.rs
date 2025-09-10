use crate::types::ReadReceiptMetadata;
use serde::{Deserialize, Serialize};

/// User read receipt information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserReadReceipt {
    /// Read receipt metadata
    #[serde(flatten)]
    pub data: ReadReceiptMetadata,
}

impl UserReadReceipt {
    pub fn new(data: ReadReceiptMetadata) -> Self {
        Self { data }
    }
}
