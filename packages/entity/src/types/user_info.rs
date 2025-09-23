use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Matrix user information for whoami endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID (MXID)
    pub user_id: String,

    /// User's display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// User's avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Whether the account is active
    pub is_active: bool,

    /// Whether the account is admin
    pub is_admin: bool,

    /// Last seen timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<DateTime<Utc>>,

    /// Account creation timestamp
    pub created_at: DateTime<Utc>,
}

impl UserInfo {
    /// Create new user info
    pub fn new(
        user_id: String,
        display_name: Option<String>,
        avatar_url: Option<String>,
        is_active: bool,
        is_admin: bool,
        last_seen: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            user_id,
            display_name,
            avatar_url,
            is_active,
            is_admin,
            last_seen,
            created_at,
        }
    }

    /// Create from User entity
    pub fn from_user(user: &crate::types::User) -> Self {
        Self {
            user_id: user.user_id.clone(),
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
            is_active: user.is_active,
            is_admin: user.is_admin,
            last_seen: user.last_seen,
            created_at: user.created_at,
        }
    }
}
