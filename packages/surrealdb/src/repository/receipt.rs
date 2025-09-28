use crate::repository::error::RepositoryError;
use serde::{Deserialize, Serialize};
use surrealdb::{Surreal, engine::any::Any};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub room_id: String,
    pub user_id: String,
    pub event_id: String,
    pub receipt_type: String,
    pub thread_id: Option<String>,
    pub timestamp: i64,
    pub is_private: bool,
    pub server_name: String,
    pub received_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ReceiptRepository {
    db: Surreal<Any>,
}

impl ReceiptRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Store a read receipt (public or private)
    pub async fn store_receipt(
        &self,
        room_id: &str,
        user_id: &str,
        event_id: &str,
        receipt_type: &str,
        thread_id: Option<&str>,
        server_name: &str,
    ) -> Result<(), RepositoryError> {
        let is_private = receipt_type == "m.read.private";
        let timestamp = Utc::now().timestamp_millis();
        let received_at = Utc::now();

        let receipt = Receipt {
            room_id: room_id.to_string(),
            user_id: user_id.to_string(),
            event_id: event_id.to_string(),
            receipt_type: receipt_type.to_string(),
            thread_id: thread_id.map(|s| s.to_string()),
            timestamp,
            is_private,
            server_name: server_name.to_string(),
            received_at,
        };

        // Use UPSERT to handle duplicate receipts
        let query = "
            UPDATE receipts SET
                event_id = $event_id,
                timestamp = $timestamp,
                thread_id = $thread_id,
                received_at = $received_at
            WHERE room_id = $room_id AND user_id = $user_id AND receipt_type = $receipt_type
            ELSE CREATE receipts SET
                room_id = $room_id,
                user_id = $user_id,
                event_id = $event_id,
                receipt_type = $receipt_type,
                thread_id = $thread_id,
                timestamp = $timestamp,
                is_private = $is_private,
                server_name = $server_name,
                received_at = $received_at
        ";

        self.db
            .query(query)
            .bind(("room_id", receipt.room_id))
            .bind(("user_id", receipt.user_id))
            .bind(("event_id", receipt.event_id))
            .bind(("receipt_type", receipt.receipt_type))
            .bind(("thread_id", receipt.thread_id))
            .bind(("timestamp", receipt.timestamp))
            .bind(("is_private", receipt.is_private))
            .bind(("server_name", receipt.server_name))
            .bind(("received_at", receipt.received_at))
            .await?;

        Ok(())
    }

    /// Get receipts for a room
    pub async fn get_room_receipts(
        &self,
        room_id: &str,
        include_private: bool,
    ) -> Result<Vec<Receipt>, RepositoryError> {
        let query = if include_private {
            "SELECT * FROM receipts WHERE room_id = $room_id ORDER BY timestamp DESC"
        } else {
            "SELECT * FROM receipts WHERE room_id = $room_id AND is_private = false ORDER BY timestamp DESC"
        };

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let receipts: Vec<Receipt> = response.take(0)?;
        Ok(receipts)
    }

    /// Get user's receipt for a specific event
    pub async fn get_user_receipt(
        &self,
        room_id: &str,
        user_id: &str,
        event_id: &str,
    ) -> Result<Option<Receipt>, RepositoryError> {
        let query = "
            SELECT * FROM receipts 
            WHERE room_id = $room_id AND user_id = $user_id AND event_id = $event_id
            ORDER BY timestamp DESC
            LIMIT 1
        ";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;

        let receipts: Vec<Receipt> = response.take(0)?;
        Ok(receipts.into_iter().next())
    }

    /// Get latest receipt for a user in a room
    pub async fn get_user_latest_receipt(
        &self,
        room_id: &str,
        user_id: &str,
        receipt_type: &str,
    ) -> Result<Option<Receipt>, RepositoryError> {
        let query = "
            SELECT * FROM receipts 
            WHERE room_id = $room_id AND user_id = $user_id AND receipt_type = $receipt_type
            ORDER BY timestamp DESC
            LIMIT 1
        ";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("receipt_type", receipt_type.to_string()))
            .await?;

        let receipts: Vec<Receipt> = response.take(0)?;
        Ok(receipts.into_iter().next())
    }
}