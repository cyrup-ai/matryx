use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Device
/// Source: spec/client/04_security_md:200-205
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub device_id: String,
    pub user_id: String,
    pub display_name: Option<String>,
    pub last_seen_ip: Option<String>,
    pub last_seen_ts: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub hidden: Option<bool>,
    pub device_keys: Option<serde_json::Value>,
    pub one_time_keys: Option<serde_json::Value>,
    pub fallback_keys: Option<serde_json::Value>,
    pub user_agent: Option<String>,
    pub initial_device_display_name: Option<String>,
}

impl Device {
    pub fn new(
        device_id: String,
        user_id: String,
        display_name: Option<String>,
        last_seen_ip: Option<String>,
        last_seen_ts: Option<i64>,
    ) -> Self {
        Self {
            device_id,
            user_id,
            display_name: display_name.clone(),
            last_seen_ip,
            last_seen_ts,
            created_at: Utc::now(),
            hidden: Some(false),
            device_keys: None,
            one_time_keys: None,
            fallback_keys: None,
            user_agent: None,
            initial_device_display_name: display_name,
        }
    }
}
