//! Matryx Matrix Client Library
//!
//! A comprehensive Matrix client implementation with SurrealDB integration
//! and real-time WebSocket support for live queries and sync.

pub mod _matrix;

use anyhow::Result;
use chrono::{DateTime, Utc};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

/// Matrix client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The homeserver URL
    pub homeserver_url: Url,
    /// HTTP client timeout in seconds
    pub timeout_secs: u64,
    /// User agent string
    pub user_agent: String,
    /// WebSocket timeout for sync operations
    pub sync_timeout_secs: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            homeserver_url: Url::parse("https://matrix.example.com").unwrap(),
            timeout_secs: 30,
            user_agent: "Matryx/0.1.0".to_string(),
            sync_timeout_secs: 30,
        }
    }
}

/// Authentication credentials for Matrix client
#[derive(Debug, Clone)]
pub struct Credentials {
    /// Matrix user ID
    pub user_id: String,
    /// Access token
    pub access_token: String,
    /// Device ID
    pub device_id: Option<String>,
}

/// Matrix client state
#[derive(Debug, Clone)]
pub struct ClientState {
    /// Current sync batch token
    pub next_batch: Option<String>,
    /// Last successful sync time
    pub last_sync: Option<DateTime<Utc>>,
    /// Connection status
    pub connected: bool,
}

/// Main Matrix client
#[derive(Debug)]
pub struct MatrixClient {
    /// HTTP client for API requests
    http_client: Client,
    /// Client configuration
    config: ClientConfig,
    /// Authentication credentials
    credentials: Option<Credentials>,
    /// Client state
    state: Arc<RwLock<ClientState>>,
}

impl MatrixClient {
    /// Create a new Matrix client with the given configuration
    pub fn new(config: ClientConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .user_agent(&config.user_agent)
            .build()?;

        let state = Arc::new(RwLock::new(ClientState {
            next_batch: None,
            last_sync: None,
            connected: false,
        }));

        Ok(Self { http_client, config, credentials: None, state })
    }

    /// Login with username and password
    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
        device_id: Option<String>,
    ) -> Result<()> {
        let response = self.login_password(username, password, device_id.as_deref()).await?;

        self.credentials = Some(Credentials {
            user_id: response.user_id,
            access_token: response.access_token,
            device_id: response.device_id,
        });

        Ok(())
    }

    /// Get the current user ID (if logged in)
    pub fn user_id(&self) -> Option<&String> {
        self.credentials.as_ref().map(|c| &c.user_id)
    }

    /// Check if client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.credentials.is_some()
    }

    /// Get the access token (if logged in)
    pub fn access_token(&self) -> Option<&str> {
        self.credentials.as_ref().map(|c| c.access_token.as_str())
    }

    /// Get the homeserver URL
    pub fn homeserver_url(&self) -> &Url {
        &self.config.homeserver_url
    }

    /// Build an authenticated request
    fn authenticated_request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Client is not authenticated"))?;

        let url = self.config.homeserver_url.join(path)?;
        let request = self
            .http_client
            .request(method, url)
            .bearer_auth(&credentials.access_token);

        Ok(request)
    }

    /// Login with username and password
    async fn login_password(
        &self,
        username: &str,
        password: &str,
        device_id: Option<&str>,
    ) -> Result<LoginResponse> {
        let url = self.config.homeserver_url.join("/_matrix/client/v3/login")?;

        let mut login_data = serde_json::json!({
            "type": "m.login.password",
            "user": username,
            "password": password
        });

        if let Some(device_id) = device_id {
            login_data["device_id"] = serde_json::Value::String(device_id.to_string());
        }

        let response = self.http_client.post(url).json(&login_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Login failed: {}", error_text));
        }

        let login_response: LoginResponse = response.json().await?;
        Ok(login_response)
    }

    /// Perform a sync operation
    pub async fn sync(
        &mut self,
        since: Option<&str>,
        timeout: Option<u64>,
    ) -> Result<SyncResponse> {
        let mut url = self.config.homeserver_url.join("/_matrix/client/v3/sync")?;

        let mut query_params = Vec::new();

        if let Some(since_token) = since {
            query_params.push(("since", since_token));
        }

        let timeout_str = timeout.unwrap_or(self.config.sync_timeout_secs).to_string();
        query_params.push(("timeout", &timeout_str));

        if !query_params.is_empty() {
            let query_string = query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            url.set_query(Some(&query_string));
        }

        let request = self.authenticated_request(reqwest::Method::GET, url.path())?;
        let response = request.send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Sync failed: {}", error_text));
        }

        let sync_response: SyncResponse = response.json().await?;

        // Update client state
        {
            let mut state = self.state.write().await;
            state.next_batch = Some(sync_response.next_batch.clone());
            state.last_sync = Some(Utc::now());
            state.connected = true;
        }

        Ok(sync_response)
    }

    /// Get current sync state
    pub async fn sync_state(&self) -> ClientState {
        self.state.read().await.clone()
    }

    /// Create a room
    pub async fn create_room(
        &self,
        room_alias: Option<&str>,
        name: Option<&str>,
        topic: Option<&str>,
    ) -> Result<String> {
        let mut room_data = serde_json::json!({});

        if let Some(alias) = room_alias {
            room_data["room_alias_name"] = serde_json::Value::String(alias.to_string());
        }

        if let Some(name) = name {
            room_data["name"] = serde_json::Value::String(name.to_string());
        }

        if let Some(topic) = topic {
            room_data["topic"] = serde_json::Value::String(topic.to_string());
        }

        let request =
            self.authenticated_request(reqwest::Method::POST, "/_matrix/client/v3/createRoom")?;
        let response = request.json(&room_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to create room: {}", error_text));
        }

        #[derive(Deserialize)]
        struct CreateRoomResponse {
            room_id: String,
        }

        let create_response: CreateRoomResponse = response.json().await?;
        Ok(create_response.room_id)
    }

    /// Send a message to a room
    pub async fn send_message(&self, room_id: &str, message: &str) -> Result<String> {
        let txn_id = uuid::Uuid::new_v4().to_string();
        let path = format!("/_matrix/client/v3/rooms/{}/send/m.room.message/{}", room_id, txn_id);

        let message_data = serde_json::json!({
            "msgtype": "m.text",
            "body": message
        });

        let request = self.authenticated_request(reqwest::Method::PUT, &path)?;
        let response = request.json(&message_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to send message: {}", error_text));
        }

        #[derive(Deserialize)]
        struct SendEventResponse {
            event_id: String,
        }

        let send_response: SendEventResponse = response.json().await?;
        Ok(send_response.event_id)
    }

    /// Join a room by ID or alias
    pub async fn join_room(&self, room_id_or_alias: &str) -> Result<String> {
        let path = format!("/_matrix/client/v3/join/{}", room_id_or_alias);

        let request = self.authenticated_request(reqwest::Method::POST, &path)?;
        let response = request.json(&serde_json::json!({})).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to join room: {}", error_text));
        }

        #[derive(Deserialize)]
        struct JoinRoomResponse {
            room_id: String,
        }

        let join_response: JoinRoomResponse = response.json().await?;
        Ok(join_response.room_id)
    }

    /// Leave a room
    pub async fn leave_room(&self, room_id: &str) -> Result<()> {
        let path = format!("/_matrix/client/v3/rooms/{}/leave", room_id);

        let request = self.authenticated_request(reqwest::Method::POST, &path)?;
        let response = request.json(&serde_json::json!({})).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to leave room: {}", error_text));
        }

        Ok(())
    }

    /// Logout from the Matrix server
    pub async fn logout(&mut self) -> Result<()> {
        if let Some(_) = &self.credentials {
            let request =
                self.authenticated_request(reqwest::Method::POST, "/_matrix/client/v3/logout")?;
            let response = request.json(&serde_json::json!({})).send().await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                return Err(anyhow::anyhow!("Logout failed: {}", error_text));
            }
        }

        self.credentials = None;

        // Reset state
        {
            let mut state = self.state.write().await;
            state.next_batch = None;
            state.last_sync = None;
            state.connected = false;
        }

        Ok(())
    }
}

/// Login response from Matrix server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    /// The fully-qualified Matrix user ID
    pub user_id: String,
    /// Access token for subsequent requests
    pub access_token: String,
    /// Device ID assigned by the server
    pub device_id: Option<String>,
    /// Well-known discovery information
    pub well_known: Option<serde_json::Value>,
}

/// Sync response from Matrix server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Token to use for next sync
    pub next_batch: String,
    /// Room updates
    pub rooms: RoomUpdates,
    /// Presence updates
    pub presence: Option<serde_json::Value>,
    /// Account data updates
    pub account_data: Option<serde_json::Value>,
    /// To-device events
    pub to_device: Option<serde_json::Value>,
    /// Device list updates
    pub device_lists: Option<serde_json::Value>,
    /// One-time keys count
    pub device_one_time_keys_count: Option<serde_json::Value>,
}

/// Room updates in sync response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUpdates {
    /// Joined rooms
    pub join: std::collections::HashMap<String, JoinedRoom>,
    /// Invited rooms
    pub invite: std::collections::HashMap<String, InvitedRoom>,
    /// Left rooms
    pub leave: std::collections::HashMap<String, LeftRoom>,
}

/// Joined room data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinedRoom {
    /// Room state
    pub state: Option<StateUpdates>,
    /// Room timeline
    pub timeline: Option<TimelineUpdates>,
    /// Ephemeral events (typing, receipts)
    pub ephemeral: Option<serde_json::Value>,
    /// Account data for this room
    pub account_data: Option<serde_json::Value>,
    /// Unread notification counts
    pub unread_notifications: Option<serde_json::Value>,
}

/// Invited room data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitedRoom {
    /// Invite state
    pub invite_state: Option<StateUpdates>,
}

/// Left room data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeftRoom {
    /// Room state
    pub state: Option<StateUpdates>,
    /// Room timeline
    pub timeline: Option<TimelineUpdates>,
    /// Account data for this room
    pub account_data: Option<serde_json::Value>,
}

/// State updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdates {
    /// State events
    pub events: Vec<Event>,
}

/// Timeline updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineUpdates {
    /// Timeline events
    pub events: Vec<Event>,
    /// Whether there are more events
    pub limited: Option<bool>,
    /// Previous batch token
    pub prev_batch: Option<String>,
}

// Re-export commonly used types from matryx_entity
pub use matryx_entity::{Event, MembershipState, Room, Session, User};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.user_agent, "Matryx/0.1.0");
        assert_eq!(config.sync_timeout_secs, 30);
    }

    #[test]
    fn test_client_creation() {
        let config = ClientConfig::default();
        let client = MatrixClient::new(config);
        assert!(client.is_ok());

        let client = client.unwrap();
        assert!(!client.is_authenticated());
        assert!(client.user_id().is_none());
        assert!(client.access_token().is_none());
    }
}
