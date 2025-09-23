use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pusher {
    pub pusher_id: String,
    pub user_id: String,
    pub kind: String,
    pub app_id: String,
    pub app_display_name: String,
    pub device_display_name: String,
    pub profile_tag: String,
    pub lang: String,
    pub data: PusherData,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PusherData {
    pub url: Option<String>,
    pub format: Option<String>,
}
