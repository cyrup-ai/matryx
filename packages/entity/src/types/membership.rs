use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::types::MembershipState;

/// Room membership record for efficient room/user relationship tracking
/// This represents the current membership state between a user and room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    /// Room ID
    pub room_id: String,

    /// User ID
    pub user_id: String,

    /// Membership state
    pub membership: MembershipState,

    /// Display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Reason for membership change
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// User who invited this member
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invited_by: Option<String>,

    /// When this membership was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,

    /// Whether this is a direct message room
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_direct: Option<bool>,

    /// Third party invite information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_invite: Option<serde_json::Value>,

    /// User server that authorized restricted room join
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_authorised_via_users_server: Option<String>,
}

impl Membership {
    pub fn new(room_id: String, user_id: String, membership: MembershipState) -> Self {
        Self {
            room_id,
            user_id,
            membership,
            display_name: None,
            avatar_url: None,
            reason: None,
            invited_by: None,
            updated_at: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        }
    }
}
