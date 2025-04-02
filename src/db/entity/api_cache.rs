use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCache {
    pub id: Option<String>,
    pub endpoint: String,
    pub parameters: Value,
    pub response_data: Value,
    pub cached_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub etag: Option<String>,
}

impl Entity for ApiCache {
    fn table_name() -> &'static str {
        "api_cache"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}