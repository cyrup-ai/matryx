use serde::{Deserialize, Serialize};

/// Filter for room events according to the Matrix specification
/// Used to filter events returned by the /messages endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventFilter {
    /// Maximum number of events to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    
    /// A list of sender IDs to exclude
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_senders: Option<Vec<String>>,
    
    /// A list of event types to exclude
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_types: Option<Vec<String>>,
    
    /// A list of senders IDs to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub senders: Option<Vec<String>>,
    
    /// A list of event types to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    
    /// Whether to include events with a URL in their content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_url: Option<bool>,
    
    /// Whether to include redundant member events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_redundant_members: Option<bool>,
    
    /// Whether to enable lazy-loading of room members
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lazy_load_members: Option<bool>,
}
