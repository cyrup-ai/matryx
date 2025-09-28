use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinRules {
    Public,
    Invite,
    Knock,
    Restricted,
    KnockRestricted,
    Private,
}

impl fmt::Display for JoinRules {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            JoinRules::Public => "public",
            JoinRules::Invite => "invite",
            JoinRules::Knock => "knock",
            JoinRules::Restricted => "restricted",
            JoinRules::KnockRestricted => "knock_restricted",
            JoinRules::Private => "private",
        };
        write!(f, "{}", s)
    }
}

impl From<String> for JoinRules {
    fn from(s: String) -> Self {
        match s.as_str() {
            "public" => JoinRules::Public,
            "invite" => JoinRules::Invite,
            "knock" => JoinRules::Knock,
            "restricted" => JoinRules::Restricted,
            "knock_restricted" => JoinRules::KnockRestricted,
            "private" => JoinRules::Private,
            _ => JoinRules::Invite, // Default fallback per Matrix spec
        }
    }
}

impl From<&str> for JoinRules {
    fn from(s: &str) -> Self {
        JoinRules::from(s.to_string())
    }
}