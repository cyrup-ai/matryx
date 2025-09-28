use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestAccess {
    CanJoin,
    Forbidden,
}

impl fmt::Display for GuestAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            GuestAccess::CanJoin => "can_join",
            GuestAccess::Forbidden => "forbidden",
        };
        write!(f, "{}", s)
    }
}

impl From<String> for GuestAccess {
    fn from(s: String) -> Self {
        match s.as_str() {
            "can_join" => GuestAccess::CanJoin,
            "forbidden" => GuestAccess::Forbidden,
            _ => GuestAccess::Forbidden, // Default fallback for security
        }
    }
}

impl From<&str> for GuestAccess {
    fn from(s: &str) -> Self {
        GuestAccess::from(s.to_string())
    }
}