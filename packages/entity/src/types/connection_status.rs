use serde::{Deserialize, Serialize};

/// Connection status for real-time client
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
