use crate::types::TypingNotification;
use serde::{Deserialize, Serialize};

/// TypingNotificationEDU
/// Source: spec/server/07-md:19-25
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingNotificationEDU {
    pub content: TypingNotification,
    pub edu_type: String,
}

impl TypingNotificationEDU {
    pub fn new(content: TypingNotification, edu_type: String) -> Self {
        Self { content, edu_type }
    }
}
