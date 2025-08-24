use crate::db::generic_dao::Entity;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Custom value entity for storing arbitrary data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomValue {
    /// Key for the custom value (hex-encoded)
    pub key: String,
    /// Value data stored as bytes
    pub value: Vec<u8>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl Entity for CustomValue {
    fn table_name() -> &'static str {
        "custom_store"
    }
    
    fn id(&self) -> Option<String> {
        Some(self.key.clone())
    }
    
    fn set_id(&mut self, id: String) {
        self.key = id;
    }
} 