use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchableMessage {
    pub id: Option<String>,
    pub message_id: String,
    pub room_id: String,
    pub sender_id: String,
    pub content: String,
    pub sent_at: DateTime<Utc>,
    pub embedding: Vec<f32>,
}

impl Entity for SearchableMessage {
    fn table_name() -> &'static str {
        "searchable_message"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}