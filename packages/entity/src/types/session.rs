use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_seen: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub user_agent: Option<String>,
    pub is_active: bool,
    pub valid: bool,
    pub puppets_user_id: Option<String>,
    pub is_guest: bool,
}

impl Session {
    pub fn new(
        session_id: String,
        user_id: String,
        device_id: String,
        access_token: String,
    ) -> Self {
        Self {
            session_id,
            user_id,
            device_id,
            access_token,
            refresh_token: None,
            created_at: Utc::now(),
            expires_at: None,
            last_seen: Some(Utc::now()),
            last_used_at: Some(Utc::now()),
            last_used_ip: None,
            user_agent: None,
            is_active: true,
            valid: true,
            puppets_user_id: None,
            is_guest: false,
        }
    }
}
