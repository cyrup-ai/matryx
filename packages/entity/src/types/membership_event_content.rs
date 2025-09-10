use crate::types::ThirdPartyInvite;
use serde::{Deserialize, Serialize};

/// Content for membership events (join/leave/invite/ban/knock)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipEventContent {
    /// Membership state
    pub membership: String,

    /// Display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Reason for the membership change
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Third party invite information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_invite: Option<ThirdPartyInvite>,
}

impl MembershipEventContent {
    pub fn new(membership: String) -> Self {
        Self {
            membership,
            display_name: None,
            avatar_url: None,
            reason: None,
            third_party_invite: None,
        }
    }
}
