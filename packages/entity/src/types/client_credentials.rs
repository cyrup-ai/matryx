use serde::{Deserialize, Serialize};

/// Authentication credentials for real-time client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeCredentials {
    /// Matrix user ID
    pub user_id: String,
    /// Access token
    pub access_token: String,
    /// Device ID
    pub device_id: String,
    /// Session ID
    pub session_id: String,
}

/// Authentication credentials for Matrix client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    /// Matrix user ID
    pub user_id: String,
    /// Access token
    pub access_token: String,
    /// Device ID
    pub device_id: Option<String>,
}
