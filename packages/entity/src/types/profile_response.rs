use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Profile response for Matrix profile API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileResponse {
    /// User ID (MXID)
    pub user_id: String,

    /// User's display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,

    /// User's avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

impl ProfileResponse {
    /// Create new profile response
    pub fn new(user_id: String, displayname: Option<String>, avatar_url: Option<String>) -> Self {
        Self { user_id, displayname, avatar_url }
    }

    /// Create from UserProfile entity
    pub fn from_user_profile(profile: &crate::types::UserProfile) -> Self {
        Self {
            user_id: profile.user_id.clone(),
            displayname: profile.display_name.clone(),
            avatar_url: profile.avatar_url.clone(),
        }
    }
}

/// Tags response for Matrix room tags API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagsResponse {
    /// Room tags mapped by tag name
    pub tags: HashMap<String, Value>,
}

impl TagsResponse {
    /// Create new tags response
    pub fn new(tags: HashMap<String, Value>) -> Self {
        Self { tags }
    }

    /// Create empty tags response
    pub fn empty() -> Self {
        Self { tags: HashMap::new() }
    }
}

/// WhoAmI response for Matrix account whoami API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhoAmIResponse {
    /// User ID (MXID)
    pub user_id: String,

    /// Device ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Whether the user is a guest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_guest: Option<bool>,
}

impl WhoAmIResponse {
    /// Create new whoami response
    pub fn new(user_id: String, device_id: Option<String>, is_guest: Option<bool>) -> Self {
        Self { user_id, device_id, is_guest }
    }

    /// Create for regular user
    pub fn user(user_id: String) -> Self {
        Self { user_id, device_id: None, is_guest: Some(false) }
    }

    /// Create for guest user
    pub fn guest(user_id: String) -> Self {
        Self { user_id, device_id: None, is_guest: Some(true) }
    }
}

/// Third-party operation types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ThirdPartyOperation {
    /// Add a new third-party identifier
    Add { medium: String, address: String, validated: bool },
    /// Remove a third-party identifier
    Remove { medium: String, address: String },
    /// Validate a third-party identifier
    Validate { medium: String, address: String, token: String },
    /// List all third-party identifiers
    List,
}

/// Third-party response for Matrix third-party ID API
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThirdPartyResponse {
    /// List of third-party identifiers
    pub threepids: Vec<ThirdPartyIdInfo>,
}

/// Third-party identifier info for API responses
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThirdPartyIdInfo {
    /// Medium type (email, msisdn)
    pub medium: String,

    /// The address (email, phone number)
    pub address: String,

    /// Whether this identifier is validated
    pub validated_at: Option<i64>,

    /// When this identifier was added
    pub added_at: i64,
}

impl ThirdPartyResponse {
    /// Create new third-party response
    pub fn new(threepids: Vec<ThirdPartyIdInfo>) -> Self {
        Self { threepids }
    }

    /// Create empty response
    pub fn empty() -> Self {
        Self { threepids: Vec::new() }
    }
}

impl ThirdPartyIdInfo {
    /// Create from ThirdPartyId entity
    pub fn from_third_party_id(third_party_id: &crate::types::ThirdPartyId) -> Self {
        Self {
            medium: third_party_id.medium.clone(),
            address: third_party_id.address.clone(),
            validated_at: third_party_id.validated_at.map(|dt| dt.timestamp()),
            added_at: third_party_id.created_at.timestamp(),
        }
    }
}
