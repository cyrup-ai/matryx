use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConnection {
    pub connection_id: String,
    pub user_id: String,
    pub device_id: String,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMembership {
    pub room_id: String,
    pub user_id: String,
    pub membership_state: String,
    pub updated_at: DateTime<Utc>,
}

pub struct WebSocketRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> WebSocketRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn register_connection(
        &self,
        user_id: &str,
        device_id: &str,
        connection_id: &str,
    ) -> Result<(), RepositoryError> {
        let connection = WebSocketConnection {
            connection_id: connection_id.to_string(),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            created_at: Utc::now(),
            last_seen: Utc::now(),
            ip_address: None,
            user_agent: None,
        };

        let _created: Option<WebSocketConnection> = self
            .db
            .create(("websocket_connection", connection_id))
            .content(connection)
            .await?;

        Ok(())
    }

    pub async fn unregister_connection(&self, connection_id: &str) -> Result<(), RepositoryError> {
        let _deleted: Option<WebSocketConnection> =
            self.db.delete(("websocket_connection", connection_id)).await?;

        Ok(())
    }

    pub async fn get_user_connections(
        &self,
        user_id: &str,
    ) -> Result<Vec<WebSocketConnection>, RepositoryError> {
        let query = "SELECT * FROM websocket_connection WHERE user_id = $user_id";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let connections: Vec<WebSocketConnection> = result.take(0)?;
        Ok(connections)
    }

    pub async fn get_connection_info(
        &self,
        connection_id: &str,
    ) -> Result<Option<WebSocketConnection>, RepositoryError> {
        let connection: Option<WebSocketConnection> =
            self.db.select(("websocket_connection", connection_id)).await?;
        Ok(connection)
    }

    pub async fn update_connection_last_seen(
        &self,
        connection_id: &str,
    ) -> Result<(), RepositoryError> {
        let query =
            "UPDATE websocket_connection SET last_seen = $last_seen WHERE id = $connection_id";
        let mut result = self
            .db
            .query(query)
            .bind(("last_seen", Utc::now()))
            .bind(("connection_id", format!("websocket_connection:{}", connection_id)))
            .await?;
        let _updated: Option<WebSocketConnection> = result.take(0)?;
        Ok(())
    }

    pub async fn get_user_memberships_for_sync(
        &self,
        user_id: &str,
    ) -> Result<Vec<RoomMembership>, RepositoryError> {
        let query = "SELECT room_id, user_id, membership AS membership_state, updated_at FROM membership WHERE user_id = $user_id AND membership IN ['join', 'invite']";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let memberships: Vec<RoomMembership> = result.take(0)?;
        Ok(memberships)
    }

    pub async fn cleanup_stale_connections(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "DELETE websocket_connection WHERE last_seen < $cutoff RETURN BEFORE";
        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;
        let deleted: Vec<WebSocketConnection> = result.take(0)?;
        Ok(deleted.len() as u64)
    }

    pub async fn broadcast_to_user_connections(
        &self,
        user_id: &str,
        message: &Value,
    ) -> Result<u32, RepositoryError> {
        // Get all active connections for the user
        let connections = self.get_user_connections(user_id).await?;

        // Store the message in the database for each active connection
        // This allows connections to retrieve messages when they reconnect
        let timestamp = chrono::Utc::now();
        let mut successful_broadcasts = 0;

        for connection in &connections {
            let message_record = serde_json::json!({
                "connection_id": connection.connection_id,
                "user_id": user_id,
                "message": message,
                "timestamp": timestamp,
                "delivered": false
            });

            // Store the message in the websocket_messages table for delivery
            let _: Option<serde_json::Value> = self
                .db
                .create("websocket_messages")
                .content(message_record)
                .await
                .map_err(|e| RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "broadcast_message".to_string(),
                })?;

            successful_broadcasts += 1;
        }

        Ok(successful_broadcasts)
    }
}
