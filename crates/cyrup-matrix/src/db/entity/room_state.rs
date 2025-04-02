use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomState {
    pub id: Option<String>,
    pub room_id: String,
    pub event_type: String,
    pub state_key: String,
    pub event: Value,
    pub updated_at: DateTime<Utc>,
}

impl Entity for RoomState {
    fn table_name() -> &'static str {
        "room_state"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}