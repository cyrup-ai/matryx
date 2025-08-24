use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::RecordId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub id: Option<RecordId>,
    pub room_id: String,
    pub receipt_type: String,
    pub thread: String,
    pub event_id: String,
    pub user_id: String,
    pub receipt_data: Value,
    pub updated_at: DateTime<Utc>,
}

impl Entity for Receipt {
    fn id(&self) -> Option<String> {
        self.id.as_ref().map(|t| t.to_string())
    }

    fn set_id(&mut self, id: String) {
        // Create a Thing from String by assuming table_name as the first part
        let parts: Vec<&str> = id.splitn(2, ':').collect();
        if parts.len() == 2 {
            let record_id = RecordId::from_table_key(Self::table_name(), parts[1]);
            self.id = Some(record_id);
        } else {
            // Fallback: use the full string as the ID part
            let record_id = RecordId::from_table_key(Self::table_name(), id.as_str());
            self.id = Some(record_id);
        }
    }

    fn table_name() -> &'static str {
        "receipts"
    }
}
