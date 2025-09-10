use crate::types::PushActionObject;
use serde::{Deserialize, Serialize};

/// Push action for Matrix push rules
/// Represents the union type object|string from the Matrix specification
/// Source: spec/client/05_advanced_features.md actions field type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PushAction {
    /// String action (like "notify", "dont_notify", "coalesce")
    String(String),
    /// Object action with additional parameters
    Object(PushActionObject),
}

impl PushAction {
    /// Create a simple string action
    pub fn string(action: impl Into<String>) -> Self {
        PushAction::String(action.into())
    }

    /// Create an object action
    pub fn object(action: PushActionObject) -> Self {
        PushAction::Object(action)
    }

    /// Create a "notify" action
    pub fn notify() -> Self {
        PushAction::String("notify".to_string())
    }

    /// Create a "dont_notify" action  
    pub fn dont_notify() -> Self {
        PushAction::String("dont_notify".to_string())
    }

    /// Create a "coalesce" action
    pub fn coalesce() -> Self {
        PushAction::String("coalesce".to_string())
    }
}
