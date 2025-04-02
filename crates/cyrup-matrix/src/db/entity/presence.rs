use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presence {
    pub id: Option<String>,
    pub user_id: String,
    pub event: Value,
    pub updated_at: DateTime<Utc>,
}

impl Entity for Presence {
    fn table_name() -> &'static str {
        "presence"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}