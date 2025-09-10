use crate::types::RoomReceipts;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ReceiptEDU
/// Source: spec/server/07-md:87-91
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptEDU {
    pub content: HashMap<String, RoomReceipts>,
    pub edu_type: String,
}

impl ReceiptEDU {
    pub fn new(content: HashMap<String, RoomReceipts>, edu_type: String) -> Self {
        Self { content, edu_type }
    }
}
