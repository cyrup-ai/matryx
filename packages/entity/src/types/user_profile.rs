use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Matrix user profile information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    /// User ID (MXID)
    pub user_id: String,

    /// User's display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// User's avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Profile creation timestamp
    pub created_at: DateTime<Utc>,

    /// Profile last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl UserProfile {
    /// Create a new user profile
    pub fn new(user_id: String) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            display_name: None,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a user profile with display name
    pub fn with_display_name(user_id: String, display_name: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            display_name,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a user profile with display name and avatar
    pub fn with_profile_data(
        user_id: String,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            display_name,
            avatar_url,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the display name
    pub fn update_display_name(&mut self, display_name: Option<String>) {
        self.display_name = display_name;
        self.updated_at = Utc::now();
    }

    /// Update the avatar URL
    pub fn update_avatar_url(&mut self, avatar_url: Option<String>) {
        self.avatar_url = avatar_url;
        self.updated_at = Utc::now();
    }
}
