use crate::db::generic_dao::Dao;
use crate::db::client::DatabaseClient;
use crate::db::entity::send_queue::SendQueueEntry;
use crate::db::error::Result;
use serde_json::Value;

/// DAO for managing the send queue
#[derive(Clone)]
pub struct SendQueueDao {
    dao: Dao<SendQueueEntry>,
}

impl SendQueueDao {
    const TABLE_NAME: &'static str = "send_queue";

    /// Create a new SendQueueDao
    pub fn new(client: DatabaseClient) -> Self {
        Self {
            dao: Dao::new(client, Self::TABLE_NAME),
        }
    }

    /// Get all requests for a room
    pub async fn get_room_requests(&self, room_id: &str) -> Result<Vec<SendQueueEntry>> {
        self.dao
            .query_with_params_raw(
                "SELECT * FROM send_queue WHERE room_id = $room_id ORDER BY priority DESC, created_at ASC",
                serde_json::json!({ "room_id": room_id }),
            )
            .await
    }

    /// Get rooms with pending requests
    pub async fn get_rooms_with_requests(&self) -> Result<Vec<String>> {
        self.dao
            .query_raw("SELECT DISTINCT room_id FROM send_queue")
            .await
    }

    /// Save a request
    pub async fn save_request(
        &self,
        room_id: &str,
        transaction_id: &str,
        created_at: i64,
        content: &Value,
        priority: usize,
        error: Option<&Value>,
    ) -> Result<SendQueueEntry> {
        let entry = SendQueueEntry {
            id: None,
            room_id: room_id.to_string(),
            transaction_id: transaction_id.to_string(),
            event_type: content
                .get("event_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            content: content.clone(),
            priority: priority as i64,
            error: error.and_then(|e| e.as_str().map(|s| s.to_string())),
            created_at: chrono::Utc::now(),
            last_attempted_at: None,
            attempts: 0,
        };

        self.dao.create(&entry).await
    }

    /// Update a request's content
    pub async fn update_request_content(
        &self,
        room_id: &str,
        transaction_id: &str,
        content: &Value,
    ) -> Result<bool> {
        let results: Vec<SendQueueEntry> = self
            .dao
            .query_with_params_raw(
                "SELECT * FROM send_queue WHERE room_id = $room_id AND transaction_id = $txn_id LIMIT 1",
                serde_json::json!({
                    "room_id": room_id,
                    "txn_id": transaction_id,
                }),
            )
            .await?;

        if let Some(mut entry) = results.into_iter().next() {
            // Update content
            entry.content = content.clone();
            entry.event_type = content
                .get("event_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            // Save changes
            self.dao.update(&entry).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Update a request's error status
    pub async fn update_request_status(
        &self,
        room_id: &str,
        transaction_id: &str,
        error: Option<&Value>,
    ) -> Result<bool> {
        let results: Vec<SendQueueEntry> = self
            .dao
            .query_with_params_raw(
                "SELECT * FROM send_queue WHERE room_id = $room_id AND transaction_id = $txn_id LIMIT 1",
                serde_json::json!({
                    "room_id": room_id,
                    "txn_id": transaction_id,
                }),
            )
            .await?;

        if let Some(mut entry) = results.into_iter().next() {
            // Update error status and attempts count
            entry.error = error.and_then(|e| e.as_str().map(|s| s.to_string()));
            entry.attempts += 1;
            entry.last_attempted_at = Some(chrono::Utc::now());

            // Save changes
            self.dao.update(&entry).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove a request
    pub async fn remove_request(&self, room_id: &str, transaction_id: &str) -> Result<bool> {
        let results: Vec<SendQueueEntry> = self
            .dao
            .query_with_params_raw(
                "SELECT * FROM send_queue WHERE room_id = $room_id AND transaction_id = $txn_id LIMIT 1",
                serde_json::json!({
                    "room_id": room_id,
                    "txn_id": transaction_id,
                }),
            )
            .await?;

        if let Some(entry) = results.into_iter().next() {
            if let Some(id) = entry.id {
                self.dao.delete(id).await?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Remove all requests for a room
    pub async fn remove_room(&self, room_id: &str) -> Result<usize> {
        let count: usize = self
            .dao
            .query_with_params_raw(
                "DELETE FROM send_queue WHERE room_id = $room_id RETURN count()",
                serde_json::json!({ "room_id": room_id }),
            )
            .await?
            .first()
            .cloned()
            .unwrap_or(0);

        Ok(count)
    }
}
