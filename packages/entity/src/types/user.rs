use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Matrix user entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// User ID (MXID)
    pub user_id: String,

    /// User's display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// User's avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Password hash
    pub password_hash: String,

    /// Account creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last seen timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<DateTime<Utc>>,

    /// Whether the account is active
    pub is_active: bool,

    /// Whether the account is admin
    pub is_admin: bool,

    /// Account data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_data: Option<serde_json::Value>,
}

impl User {
    /// Create a new user
    pub fn new(user_id: String, password_hash: String) -> Self {
        Self {
            user_id,
            display_name: None,
            avatar_url: None,
            password_hash,
            created_at: Utc::now(),
            last_seen: None,
            is_active: true,
            is_admin: false,
            account_data: None,
        }
    }

    /// Create a new admin user
    pub fn new_admin(user_id: String, password_hash: String) -> Self {
        Self {
            user_id,
            display_name: None,
            avatar_url: None,
            password_hash,
            created_at: Utc::now(),
            last_seen: None,
            is_active: true,
            is_admin: true,
            account_data: None,
        }
    }
}
