use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Matrix account data entity as defined in the Matrix specification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountData {
    /// User ID this account data belongs to
    pub user_id: String,

    /// Account data type
    pub account_data_type: String,

    /// Account data content
    pub content: serde_json::Value,

    /// Room ID if this is room-specific account data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,

    /// Timestamp when this account data was created
    pub created_at: DateTime<Utc>,

    /// Timestamp when this account data was last updated
    pub updated_at: DateTime<Utc>,
}

impl AccountData {
    /// Create new global account data
    pub fn new_global(
        user_id: String,
        account_data_type: String,
        content: serde_json::Value,
    ) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            account_data_type,
            content,
            room_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create new room-specific account data
    pub fn new_room(
        user_id: String,
        room_id: String,
        account_data_type: String,
        content: serde_json::Value,
    ) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            account_data_type,
            content,
            room_id: Some(room_id),
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if this is global account data
    pub fn is_global(&self) -> bool {
        self.room_id.is_none()
    }

    /// Check if this is room-specific account data
    pub fn is_room_specific(&self) -> bool {
        self.room_id.is_some()
    }
}
