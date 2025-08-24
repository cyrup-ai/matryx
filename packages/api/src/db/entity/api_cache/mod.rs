use crate::db::generic_dao::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// API cache entity for storing cached API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCacheEntry {
    /// Entity ID (auto-generated)
    pub id: Option<String>,
    /// API endpoint
    pub endpoint: String,
    /// Request parameters
    pub parameters: Value,
    /// Response data
    pub response_data: Value,
    /// When this cache entry was created/updated
    pub cached_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub etag: Option<String>,
}

impl Entity for ApiCacheEntry {
    fn id(&self) -> Option<String> {
        self.id.clone()
    }

    fn table_name() -> &'static str {
        "api_cache"
    }

    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}
