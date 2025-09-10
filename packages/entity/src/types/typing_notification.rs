use serde::{Deserialize, Serialize};

/// TypingNotification
/// Source: spec/server/07-md:27-31
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingNotification {
    pub room_id: String,
    pub typing: bool,
    pub user_id: String,
}

impl TypingNotification {
    pub fn new(room_id: String, typing: bool, user_id: String) -> Self {
        Self { room_id, typing, user_id }
    }
}
