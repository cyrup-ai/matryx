use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToDeviceMessage {
    pub id: String,
    pub sender: String,
    pub event_type: String,
    pub content: serde_json::Value,
    pub target_user_id: String,
    pub target_device_id: Option<String>,
    pub txn_id: String,
    pub created_at: DateTime<Utc>,
}

pub struct MessagingRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> MessagingRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Check if a transaction ID already exists for a sender
    pub async fn check_transaction_exists(
        &self,
        sender: &str,
        txn_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "SELECT id FROM to_device_messages WHERE sender = $sender AND txn_id = $txn_id";
        let mut response = self
            .db
            .query(query)
            .bind(("sender", sender.to_string()))
            .bind(("txn_id", txn_id.to_string()))
            .await?;

        let existing: Option<String> = response.take(0)?;
        Ok(existing.is_some())
    }

    /// Store a to-device message
    pub async fn store_to_device_message(
        &self,
        message: &ToDeviceMessage,
    ) -> Result<(), RepositoryError> {
        let _: Option<ToDeviceMessage> = self
            .db
            .create(("to_device_messages", &message.id))
            .content(message.clone())
            .await?;

        Ok(())
    }

    /// Get to-device messages for a user
    pub async fn get_to_device_messages(
        &self,
        user_id: &str,
        device_id: Option<&str>,
    ) -> Result<Vec<ToDeviceMessage>, RepositoryError> {
        let query = if let Some(_device_id) = device_id {
            "SELECT * FROM to_device_messages WHERE target_user_id = $user_id AND (target_device_id IS NULL OR target_device_id = $device_id) ORDER BY created_at"
        } else {
            "SELECT * FROM to_device_messages WHERE target_user_id = $user_id ORDER BY created_at"
        };

        let mut result = if let Some(device_id) = device_id {
            self.db
                .query(query)
                .bind(("user_id", user_id.to_string()))
                .bind(("device_id", device_id.to_string()))
                .await?
        } else {
            self.db
                .query(query)
                .bind(("user_id", user_id.to_string()))
                .await?
        };

        let messages: Vec<ToDeviceMessage> = result.take(0)?;
        Ok(messages)
    }

    /// Delete to-device messages after delivery
    pub async fn delete_to_device_messages(
        &self,
        message_ids: &[String],
    ) -> Result<(), RepositoryError> {
        for message_id in message_ids {
            let _: Option<ToDeviceMessage> = self
                .db
                .delete(("to_device_messages", message_id))
                .await?;
        }

        Ok(())
    }

    /// Get message statistics for a user
    pub async fn get_message_statistics(
        &self,
        user_id: &str,
    ) -> Result<serde_json::Value, RepositoryError> {
        let query = "
            SELECT 
                count() as total_messages,
                count(target_device_id IS NULL) as broadcast_messages,
                count(target_device_id IS NOT NULL) as device_messages
            FROM to_device_messages 
            WHERE target_user_id = $user_id
            GROUP ALL
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let stats: Vec<serde_json::Value> = result.take(0)?;
        Ok(stats.into_iter().next().unwrap_or(serde_json::json!({})))
    }
}