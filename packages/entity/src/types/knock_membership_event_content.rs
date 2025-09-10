use serde::{Deserialize, Serialize};

/// Knock membership event content for Matrix room membership
/// Represents the content of a knock membership event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockMembershipEventContent {
    pub membership: String,
    pub reason: Option<String>,
    pub displayname: Option<String>,
    pub avatar_url: Option<String>,
}

impl KnockMembershipEventContent {
    pub fn new() -> Self {
        Self {
            membership: "knock".to_string(),
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

impl Default for KnockMembershipEventContent {
    fn default() -> Self {
        Self::new()
    }
}
