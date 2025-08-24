use crate::db::generic_dao::Entity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};

/// Entity representation of a key-value pair for Matrix storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    /// Entity ID (auto-generated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Key for this entry
    pub key: String,
    /// Value for this entry (raw bytes stored as Base64)
    pub value: Value,
    /// The type of the value for deserialization
    pub value_type: String,
    /// When this key-value pair was created
    pub created_at: DateTime<Utc>,
    /// When this key-value pair was last updated
    pub updated_at: DateTime<Utc>,
}

impl Entity for KeyValue {
    fn table_name() -> &'static str {
        "key_value"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl KeyValue {
    /// Create a new KeyValue entry
    pub fn new(key: impl Into<String>, value: Value, value_type: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            key: key.into(),
            value,
            value_type: value_type.into(),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Create a new binary KeyValue entry (base64 encoded)
    pub fn new_binary(key: impl Into<String>, value: &[u8]) -> Self {
        Self {
            id: None,
            key: key.into(),
            value: Value::String(BASE64.encode(value)),
            value_type: "binary".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
    
    /// Get the binary value (if this is a binary value)
    pub fn binary_value(&self) -> Option<Vec<u8>> {
        if self.value_type == "binary" {
            if let Value::String(base64_str) = &self.value {
                return BASE64.decode(base64_str).ok();
            }
        }
        None
    }
    
    /// Set a binary value
    pub fn set_binary_value(&mut self, value: &[u8]) {
        self.value = Value::String(BASE64.encode(value));
        self.value_type = "binary".into();
        self.updated_at = Utc::now();
    }
    
    /// Get the value as bytes
    pub fn value_bytes(&self) -> Vec<u8> {
        if let Value::String(base64_str) = &self.value {
            BASE64.decode(base64_str).unwrap_or_default()
        } else {
            Vec::new()
        }
    }
    
    /// Update the value
    pub fn update_value(&mut self, value: impl AsRef<[u8]>) {
        self.value = Value::String(BASE64.encode(value.as_ref()));
        self.updated_at = Utc::now();
    }
}