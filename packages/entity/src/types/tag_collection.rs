use crate::types::RoomTag;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TagCollection
/// Source: spec/client/06_user_md:70
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCollection {
    pub tags: HashMap<String, RoomTag>,
}

impl TagCollection {
    pub fn new(tags: HashMap<String, RoomTag>) -> Self {
        Self { tags }
    }
}
