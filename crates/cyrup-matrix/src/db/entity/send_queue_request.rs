use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::sql::Thing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendQueueRequest {
    pub id: Option<Thing>,
    pub room_id: String,
    pub transaction_id: String,
    pub created_at: i64,  // Milliseconds since Unix epoch
    pub kind: String,
    pub content: Value,
    pub priority: usize,
    pub error: Option<Value>,
    pub updated_at: DateTime<Utc>,
}

impl Entity for SendQueueRequest {
    fn id(&self) -> Option<String> {
        self.id.as_ref().map(|t| t.to_string())
    }
    
    fn set_id(&mut self, id: String) {
        // Create a Thing from String by assuming table_name as the first part
        let parts: Vec<&str> = id.splitn(2, ':').collect();
        if parts.len() == 2 {
            let thing = Thing::from((Self::table_name(), parts[1]));
            self.id = Some(thing);
        } else {
            // Fallback: use the full string as the ID part
            let thing = Thing::from((Self::table_name(), id.as_str()));
            self.id = Some(thing);
        }
    }
    
    fn table_name() -> &'static str {
        "send_queue_request"
    }
}