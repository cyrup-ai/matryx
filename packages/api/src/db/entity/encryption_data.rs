use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionData {
    pub id: Option<String>,
    pub user_id: String,
    pub device_id: String,
    pub keys: Value,
    pub signatures: Value,
    pub verification_status: String,
    pub updated_at: DateTime<Utc>,
}

impl Entity for EncryptionData {
    fn table_name() -> &'static str {
        "encryption_data"
    }

    fn id(&self) -> Option<String> {
        self.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}
