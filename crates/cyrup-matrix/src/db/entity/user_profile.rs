use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub device_id: String,
    pub last_seen: DateTime<Utc>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: Option<String>,
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub presence: String,
    pub last_active: DateTime<Utc>,
    pub devices: Vec<Device>,
    pub settings: Value,
}

impl Entity for UserProfile {
    fn table_name() -> &'static str {
        "user_profile"
    }
    
    fn id(&self) -> Option<String> {
        self.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}