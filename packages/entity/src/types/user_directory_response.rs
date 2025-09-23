use serde::{Deserialize, Serialize};

/// Response for user directory search
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserDirectoryResponse {
    /// List of matching users
    pub results: Vec<UserDirectoryEntry>,
    /// Whether the results were limited due to the query limit
    pub limited: bool,
}

/// User entry in directory search results
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserDirectoryEntry {
    /// The user ID
    pub user_id: String,
    /// The user's display name
    pub display_name: Option<String>,
    /// The user's avatar URL
    pub avatar_url: Option<String>,
    /// Whether the user is a guest user
    pub is_guest: bool,
}
