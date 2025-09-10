use serde::{Deserialize, Serialize};

/// Matrix room membership states as defined in the Matrix specification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MembershipState {
    /// User has been invited to the room
    Invite,
    /// User has joined the room
    Join,
    /// User has left the room
    Leave,
    /// User has been banned from the room
    Ban,
    /// User has knocked on the room (requesting to join)
    Knock,
}

impl std::fmt::Display for MembershipState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MembershipState::Invite => write!(f, "invite"),
            MembershipState::Join => write!(f, "join"),
            MembershipState::Leave => write!(f, "leave"),
            MembershipState::Ban => write!(f, "ban"),
            MembershipState::Knock => write!(f, "knock"),
        }
    }
}

impl From<String> for MembershipState {
    fn from(s: String) -> Self {
        match s.as_str() {
            "invite" => MembershipState::Invite,
            "join" => MembershipState::Join,
            "leave" => MembershipState::Leave,
            "ban" => MembershipState::Ban,
            "knock" => MembershipState::Knock,
            _ => MembershipState::Leave, // Default fallback
        }
    }
}
