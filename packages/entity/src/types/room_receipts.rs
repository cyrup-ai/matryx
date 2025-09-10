use crate::types::UserReadReceipt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RoomReceipts
/// Source: spec/server/07-md:93-97
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomReceipts {
    #[serde(rename = "m.read")]
    pub read: HashMap<String, UserReadReceipt>,
}

impl RoomReceipts {
    pub fn new(read: HashMap<String, UserReadReceipt>) -> Self {
        Self { read }
    }
}
