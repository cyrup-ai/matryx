use serde::{Deserialize, Serialize};

/// Invite user request
/// Source: spec/client/02_rooms_md:523-524
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteUserRequest {
    pub user_id: String,
    pub reason: Option<String>,
}

impl InviteUserRequest {
    pub fn new(user_id: String, reason: Option<String>) -> Self {
        Self { user_id, reason }
    }
}
