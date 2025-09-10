use serde::{Deserialize, Serialize};

/// RoomTag
/// Source: spec/client/06_user_md:74
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomTag {
    pub order: f64,
}

impl RoomTag {
    pub fn new(order: f64) -> Self {
        Self { order }
    }
}
