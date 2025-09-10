use serde::{Deserialize, Serialize};

/// Push condition value for Matrix push rules
/// Represents the value field in push conditions which can be string, number, or boolean
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PushConditionValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl PushConditionValue {
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub fn number(value: f64) -> Self {
        Self::Number(value)
    }

    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }
}
