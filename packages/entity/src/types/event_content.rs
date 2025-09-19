use crate::types::{
    AuthenticationContent,
    DirectToDeviceContent,
    EncryptedContent,
    InviteMembershipEventContent,
    KnockMembershipEventContent,
    LeaveMembershipEventContent,
    MembershipEventContent,
    ServerNoticeContent,
    StickerContent,
    ThirdPartyInviteEventContent,
};
use serde::{Deserialize, Serialize};

/// Event content for Matrix events - typed enum for different event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventContent {
    /// Membership events (m.room.member)
    Membership(MembershipEventContent),

    /// Invite membership events
    InviteMembership(InviteMembershipEventContent),

    /// Knock membership events
    KnockMembership(KnockMembershipEventContent),

    /// Leave membership events
    LeaveMembership(LeaveMembershipEventContent),

    /// Third party invite events
    ThirdPartyInvite(ThirdPartyInviteEventContent),

    /// Encrypted content
    Encrypted(EncryptedContent),

    /// Direct-to-device content
    DirectToDevice(DirectToDeviceContent),

    /// Authentication content
    Authentication(AuthenticationContent),

    /// Sticker content (m.sticker)
    Sticker(StickerContent),

    /// Server notice content (m.server_notice)
    ServerNotice(ServerNoticeContent),

    /// Generic fallback for unknown event types
    Unknown(serde_json::Value),
}

impl EventContent {
    /// Create a membership event content
    pub fn membership(membership: String) -> Self {
        Self::Membership(MembershipEventContent::new(membership))
    }

    /// Create unknown content from JSON value
    pub fn unknown(value: serde_json::Value) -> Self {
        Self::Unknown(value)
    }

    /// Get as object for JSON manipulation
    pub fn as_object(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        match self {
            Self::Unknown(serde_json::Value::Object(obj)) => Some(obj),
            _ => None,
        }
    }

    /// Get field from content if it's an object
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.as_object()?.get(key)
    }

    /// Check if content is null
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Unknown(serde_json::Value::Null))
    }

    /// Check if content is an object
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Unknown(serde_json::Value::Object(_)))
    }
}

impl Default for EventContent {
    fn default() -> Self {
        Self::Unknown(serde_json::Value::Null)
    }
}
