use serde::{Deserialize, Serialize};

/// Filter for room events and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    /// Maximum number of events to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,

    /// Event types to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,

    /// Event types to exclude
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_types: Option<Vec<String>>,

    /// Senders to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub senders: Option<Vec<String>>,

    /// Senders to exclude
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_senders: Option<Vec<String>>,
}

impl Filter {
    pub fn new() -> Self {
        Self {
            limit: None,
            types: None,
            not_types: None,
            senders: None,
            not_senders: None,
        }
    }
}
