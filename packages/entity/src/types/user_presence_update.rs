use serde::{Deserialize, Serialize};

/// User presence update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPresenceUpdate {
    /// User ID
    pub user_id: String,

    /// Presence state
    pub presence: String,

    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_msg: Option<String>,

    /// Last active timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_ago: Option<i64>,

    /// Currently active status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currently_active: Option<bool>,
}

impl UserPresenceUpdate {
    pub fn new(user_id: String, presence: String) -> Self {
        Self {
            user_id,
            presence,
            status_msg: None,
            last_active_ago: None,
            currently_active: None,
        }
    }
}
