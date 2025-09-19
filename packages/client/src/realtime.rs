//! Real-time Matrix client with WebSocket and LiveQuery integration
//!
//! This module provides a high-level real-time Matrix client that combines
//! traditional Matrix Client-Server API with SurrealDB LiveQuery for superior
//! real-time performance and reduced server load.

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use matryx_entity::{Event, Membership};
use matryx_surrealdb::repository::{
    EventRepository,
    MembershipRepository,
    RoomRepository,
    SessionRepository,
    UserRepository,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};
use url::Url;

use crate::sync::{LiveQuerySync, SyncState, SyncUpdate};

/// Real-time Matrix client configuration
#[derive(Debug, Clone)]
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
            homeserver_url: Url::parse("https://matrix.example.com").unwrap(),
            websocket_url: None,
            surrealdb_url: Url::parse("ws://localhost:8000").unwrap(),
            timeout_secs: 30,
            max_reconnect_attempts: 5,
            reconnect_delay_secs: 5,
        }
    }
}

/// Authentication credentials for real-time client
#[derive(Debug, Clone)]
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

/// Connection status for real-time client
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// Disconnected from all services
    Disconnected,
    /// Connecting to services
    Connecting,
    /// Connected to HTTP API only
    HttpOnly,
    /// Connected to HTTP API and SurrealDB
    DatabaseConnected,
    /// Fully connected (HTTP, SurrealDB, and WebSocket if configured)
    FullyConnected,
    /// Connection error
    Error(String),
}

/// Real-time Matrix client event
#[derive(Debug, Clone)]
pub enum RealtimeEvent {
    /// Connection status changed
    ConnectionStatusChanged {
        old_status: ConnectionStatus,
        new_status: ConnectionStatus,
    },
    /// Sync update received
    SyncUpdate(SyncUpdate),
    /// Room event received
    RoomEvent { room_id: String, event: Event },
    /// Membership changed
    MembershipChanged {
        room_id: String,
        user_id: String,
        membership: Membership,
    },
    /// Typing notification
    TypingNotification { room_id: String, user_id: String, typing: bool },
    /// Read receipt
    ReadReceipt {
        room_id: String,
        user_id: String,
        event_id: String,
        timestamp: u64,
    },
    /// Presence update
    PresenceUpdate {
        user_id: String,
        presence: String,
        status_msg: Option<String>,
        last_active_ago: Option<u64>,
    },
    /// Device list update
    DeviceListUpdate { changed: Vec<String>, left: Vec<String> },
    /// Error occurred
    Error { message: String, recoverable: bool },
}

/// Real-time Matrix client with LiveQuery integration
pub struct RealtimeMatrixClient {
    /// Client configuration
    config: RealtimeConfig,
    /// Authentication credentials
    credentials: Option<RealtimeCredentials>,
    /// Current connection status
    status: Arc<RwLock<ConnectionStatus>>,
    /// HTTP client for Matrix API
    http_client: reqwest::Client,
    /// SurrealDB connection
    db: Option<surrealdb::Surreal<surrealdb::engine::remote::ws::Client>>,
    /// SurrealDB repositories
    event_repo: Option<EventRepository<surrealdb::engine::remote::ws::Client>>,
    membership_repo: Option<MembershipRepository<surrealdb::engine::remote::ws::Client>>,
    room_repo: Option<RoomRepository>,
    user_repo: Option<UserRepository>,
    session_repo: Option<SessionRepository>,
    /// LiveQuery sync manager
    sync_manager: Option<LiveQuerySync>,
    /// Event broadcast channel
    event_sender: broadcast::Sender<RealtimeEvent>,
    /// Event receiver
    event_receiver: broadcast::Receiver<RealtimeEvent>,
    /// WebSocket connection
    websocket_tx: Option<
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
    >,
}

impl RealtimeMatrixClient {
    /// Create a new real-time Matrix client
    pub fn new(config: RealtimeConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .user_agent("Matryx-Realtime-Client/0.1.0")
            .build()?;

        let (event_sender, event_receiver) = broadcast::channel(1000);

        Ok(Self {
            config,
            credentials: None,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            http_client,
            db: None,
            event_repo: None,
            membership_repo: None,
            room_repo: None,
            user_repo: None,
            session_repo: None,
            sync_manager: None,
            event_sender,
            event_receiver,
            websocket_tx: None,
        })
    }

    /// Login and establish real-time connections
    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
        device_id: Option<String>,
    ) -> Result<()> {
        self.set_status(ConnectionStatus::Connecting).await;

        // Step 1: HTTP login to Matrix server
        let login_response = self.http_login(username, password, device_id.as_deref()).await?;

        self.credentials = Some(RealtimeCredentials {
            user_id: login_response.user_id.clone(),
            access_token: login_response.access_token.clone(),
            device_id: login_response
                .device_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            session_id: uuid::Uuid::new_v4().to_string(),
        });

        self.set_status(ConnectionStatus::HttpOnly).await;

        // Step 2: Connect to SurrealDB
        self.connect_surrealdb().await?;
        self.set_status(ConnectionStatus::DatabaseConnected).await;

        // Step 3: Initialize LiveQuery sync
        self.initialize_sync().await?;

        // Note: Sync stream connection handled internally by LiveQuerySync

        // Step 4: Connect WebSocket if configured
        if self.config.websocket_url.is_some() {
            self.connect_websocket().await?;
        }

        self.set_status(ConnectionStatus::FullyConnected).await;

        info!("Real-time Matrix client logged in successfully: {}", login_response.user_id);
        Ok(())
    }

    /// Perform HTTP login to Matrix server
    async fn http_login(
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

    /// Connect to SurrealDB and initialize repositories
    async fn connect_surrealdb(&mut self) -> Result<()> {
        use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};

        let db = Surreal::new::<Ws>(&self.config.surrealdb_url.to_string()).await?;

        // Authenticate with SurrealDB (using root for now, should be configurable)
        db.signin(Root { username: "root", password: "root" }).await?;

        // Use the matryx namespace and matrix database
        db.use_ns("matryx").use_db("matrix").await?;

        // Store database connection
        self.db = Some(db.clone());

        // Initialize repositories with appropriate connection types
        self.event_repo = Some(EventRepository::new(db.clone()));
        self.membership_repo = Some(MembershipRepository::new(db.clone()));

        // For now, skip the repositories that require Surreal<Any> to focus on LiveQuery functionality
        // TODO: Fix repository connection types in a separate task
        self.room_repo = None;
        self.user_repo = None;
        self.session_repo = None;

        debug!("Connected to SurrealDB and initialized repositories");
        Ok(())
    }

    /// Initialize LiveQuery sync manager
    async fn initialize_sync(&mut self) -> Result<()> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No credentials available"))?;

        let _event_repo = self
            .event_repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Event repository not initialized"))?;

        let _membership_repo = self
            .membership_repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Membership repository not initialized"))?;

        let _room_repo = self
            .room_repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Room repository not initialized"))?;

        let _user_repo = self
            .user_repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("User repository not initialized"))?;

        let db = self
            .db
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not initialized"))?;

        let sync_manager = LiveQuerySync::new(credentials.user_id.clone(), db.clone());

        // Start the sync manager
        sync_manager.start().await?;

        // Store sync manager
        self.sync_manager = Some(sync_manager);

        // Note: Sync stream connection will be established after initialization
        // The sync manager is now running and will handle LiveQuery subscriptions internally

        debug!("Initialized LiveQuery sync manager");
        Ok(())
    }



    /// Connect WebSocket for additional real-time features
    async fn connect_websocket(&mut self) -> Result<()> {
        if let Some(ws_url) = &self.config.websocket_url {
            let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url.as_str()).await?;
            let (ws_tx, mut ws_rx) = ws_stream.split();

            self.websocket_tx = Some(ws_tx);

            // Handle incoming WebSocket messages
            let event_sender = self.event_sender.clone();
            tokio::spawn(async move {
                use futures_util::StreamExt;
                while let Some(message) = ws_rx.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            debug!("Received WebSocket message: {}", text);
                            // Parse and handle WebSocket messages
                            // This would include typing notifications, read receipts, etc.
                        },
                        Ok(Message::Close(_)) => {
                            info!("WebSocket connection closed");
                            break;
                        },
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            let _ = event_sender.send(RealtimeEvent::Error {
                                message: format!("WebSocket error: {}", e),
                                recoverable: true,
                            });
                        },
                        _ => {},
                    }
                }
            });

            debug!("Connected to WebSocket");
        }

        Ok(())
    }

    /// Set connection status and notify listeners
    async fn set_status(&self, new_status: ConnectionStatus) {
        let old_status = {
            let mut status = self.status.write().await;
            let old = status.clone();
            *status = new_status.clone();
            old
        };

        if old_status != new_status {
            let event = RealtimeEvent::ConnectionStatusChanged { old_status, new_status };
            let _ = self.event_sender.send(event);
        }
    }

    /// Get current connection status
    pub async fn connection_status(&self) -> ConnectionStatus {
        self.status.read().await.clone()
    }

    /// Get event stream for real-time updates
    pub fn event_stream(&self) -> impl futures_util::Stream<Item = RealtimeEvent> {
        let receiver = self.event_sender.subscribe();
        tokio_stream::wrappers::BroadcastStream::new(receiver).filter_map(|result| {
            async move {
                match result {
                    Ok(event) => Some(event),
                    Err(e) => {
                        warn!("Error in event stream: {}", e);
                        None
                    },
                }
            }
        })
    }

    /// Send a message to a room
    pub async fn send_message(&self, room_id: &str, message: &str) -> Result<String> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let txn_id = uuid::Uuid::new_v4().to_string();
        let path = format!("/_matrix/client/v3/rooms/{}/send/m.room.message/{}", room_id, txn_id);
        let url = self.config.homeserver_url.join(&path)?;

        let message_data = serde_json::json!({
            "msgtype": "m.text",
            "body": message
        });

        let response = self
            .http_client
            .put(url)
            .bearer_auth(&credentials.access_token)
            .json(&message_data)
            .send()
            .await?;

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

    /// Join a room
    pub async fn join_room(&self, room_id_or_alias: &str) -> Result<String> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let path = format!("/_matrix/client/v3/join/{}", room_id_or_alias);
        let url = self.config.homeserver_url.join(&path)?;

        let response = self
            .http_client
            .post(url)
            .bearer_auth(&credentials.access_token)
            .json(&serde_json::json!({}))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to join room: {}", error_text));
        }

        #[derive(Deserialize)]
        struct JoinRoomResponse {
            room_id: String,
        }

        let join_response: JoinRoomResponse = response.json().await?;

        // Subscribe to the new room for real-time updates
        if let Some(sync_manager) = &self.sync_manager {
            sync_manager.subscribe_to_room(&join_response.room_id).await?;
        }

        Ok(join_response.room_id)
    }

    /// Leave a room
    pub async fn leave_room(&self, room_id: &str) -> Result<()> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let path = format!("/_matrix/client/v3/rooms/{}/leave", room_id);
        let url = self.config.homeserver_url.join(&path)?;

        let response = self
            .http_client
            .post(url)
            .bearer_auth(&credentials.access_token)
            .json(&serde_json::json!({}))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to leave room: {}", error_text));
        }

        Ok(())
    }

    /// Get current sync state
    pub async fn sync_state(&self) -> Option<SyncState> {
        if let Some(sync_manager) = &self.sync_manager {
            Some(sync_manager.get_sync_state().await)
        } else {
            None
        }
    }

    /// Logout and cleanup connections
    pub async fn logout(&mut self) -> Result<()> {
        if let Some(credentials) = &self.credentials {
            // HTTP logout
            let path = "/_matrix/client/v3/logout";
            let url = self.config.homeserver_url.join(path)?;

            let response = self
                .http_client
                .post(url)
                .bearer_auth(&credentials.access_token)
                .json(&serde_json::json!({}))
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                warn!("Logout failed: {}", error_text);
            }
        }

        // Stop sync manager
        if let Some(sync_manager) = &self.sync_manager {
            sync_manager.stop().await?;
        }

        // Close WebSocket
        if let Some(mut ws_tx) = self.websocket_tx.take() {
            let _ = ws_tx.close().await;
        }

        // Clear credentials and state
        self.credentials = None;
        self.sync_manager = None;
        self.event_repo = None;
        self.membership_repo = None;
        self.room_repo = None;
        self.user_repo = None;
        self.session_repo = None;

        self.set_status(ConnectionStatus::Disconnected).await;

        info!("Logged out and cleaned up connections");
        Ok(())
    }

    /// Get user ID if authenticated
    pub fn user_id(&self) -> Option<&str> {
        self.credentials.as_ref().map(|c| c.user_id.as_str())
    }

    /// Check if client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.credentials.is_some()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realtime_config_default() {
        let config = RealtimeConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_reconnect_attempts, 5);
        assert_eq!(config.reconnect_delay_secs, 5);
    }

    #[tokio::test]
    async fn test_realtime_client_creation() {
        let config = RealtimeConfig::default();
        let client = RealtimeMatrixClient::new(config);
        assert!(client.is_ok());

        let client = client.unwrap();
        assert!(!client.is_authenticated());
        assert!(client.user_id().is_none());
        assert_eq!(client.connection_status().await, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_connection_status_changes() {
        let config = RealtimeConfig::default();
        let client = RealtimeMatrixClient::new(config).unwrap();

        assert_eq!(client.connection_status().await, ConnectionStatus::Disconnected);

        client.set_status(ConnectionStatus::Connecting).await;
        assert_eq!(client.connection_status().await, ConnectionStatus::Connecting);

        client.set_status(ConnectionStatus::FullyConnected).await;
        assert_eq!(client.connection_status().await, ConnectionStatus::FullyConnected);
    }
}
