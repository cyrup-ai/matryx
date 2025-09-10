use serde::{Deserialize, Serialize};

/// Leave membership event content for Matrix room membership
/// Represents the content of a leave membership event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveMembershipEventContent {
    pub membership: String,
    pub reason: Option<String>,
    pub displayname: Option<String>,
    pub avatar_url: Option<String>,
}

impl LeaveMembershipEventContent {
    pub fn new() -> Self {
        Self {
            membership: "leave".to_string(),
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

impl Default for LeaveMembershipEventContent {
    fn default() -> Self {
        Self::new()
    }
}
