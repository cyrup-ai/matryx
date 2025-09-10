use serde::{Deserialize, Serialize};

/// Invite membership event content for Matrix room membership
/// Represents the content of an invite membership event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteMembershipEventContent {
    pub membership: String,
    pub reason: Option<String>,
    pub displayname: Option<String>,
    pub avatar_url: Option<String>,
}

impl InviteMembershipEventContent {
    pub fn new() -> Self {
        Self {
            membership: "invite".to_string(),
            reason: None,
            displayname: None,
            avatar_url: None,
        }
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }
}

impl Default for InviteMembershipEventContent {
    fn default() -> Self {
        Self::new()
    }
}
