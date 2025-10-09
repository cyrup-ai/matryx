use serde::{Deserialize, Serialize};
use url::Url;

/// Real-time Matrix client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeConfig {
    /// Homeserver URL for HTTP API
    pub homeserver_url: Url,
    /// WebSocket URL for real-time connections
    pub websocket_url: Option<Url>,
    /// SurrealDB connection URL
    pub surrealdb_url: Url,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
    /// Reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Reconnection delay in seconds
    pub reconnect_delay_secs: u64,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            homeserver_url: Url::parse("https://matrix.org")
                .unwrap_or_else(|e| panic!("BUG: Default homeserver URL 'https://matrix.org' failed to parse: {}", e)),
            websocket_url: None,
            surrealdb_url: Url::parse("ws://127.0.0.1:8000")
                .unwrap_or_else(|e| panic!("BUG: Default SurrealDB URL 'ws://127.0.0.1:8000' failed to parse: {}", e)),
            timeout_secs: 30,
            max_reconnect_attempts: 5,
            reconnect_delay_secs: 5,
        }
    }
}
