use serde::{Deserialize, Serialize};

/// Push action object for Matrix push rules
/// Represents object-type push actions with additional parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushActionObject {
    pub set_tweak: Option<String>,
    pub value: Option<String>,
}

impl PushActionObject {
    pub fn new(set_tweak: Option<String>, value: Option<String>) -> Self {
        Self { set_tweak, value }
    }

    pub fn sound(sound: String) -> Self {
        Self {
            set_tweak: Some("sound".to_string()),
            value: Some(sound),
        }
    }

    pub fn highlight() -> Self {
        Self {
            set_tweak: Some("highlight".to_string()),
            value: Some("true".to_string()),
        }
    }
}
