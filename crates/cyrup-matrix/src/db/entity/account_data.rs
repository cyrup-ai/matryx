use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountData {
    pub id: Option<String>,
    pub event_type: String,
    pub room_id: Option<String>,
    pub event: Value,
    pub updated_at: DateTime<Utc>,
}

impl Entity for AccountData {
    fn table_name() -> &'static str {
        "account_data"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}