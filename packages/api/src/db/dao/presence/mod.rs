use crate::db::client::DatabaseClient;
use crate::db::entity::presence::PresenceData;
use crate::db::error::Result;
use crate::db::generic_dao::Dao;
use crate::future::MatrixFuture;
use chrono::Utc;
use serde_json::{json, Value};

/// DAO for presence data
#[derive(Clone)]
pub struct PresenceDao {
    dao: Dao<PresenceData>,
}

impl PresenceDao {
    const TABLE_NAME: &'static str = "presence";

    /// Create a new PresenceDao
    pub fn new(client: DatabaseClient) -> Self {
        Self {
            dao: Dao::new(client, Self::TABLE_NAME),
        }
    }

    /// Get presence for a user
    pub async fn get_presence(&self, user_id: &str) -> Result<Option<PresenceData>> {
        self.dao.find_by_field("user_id", user_id).await
    }

    /// Save presence for a user
    pub async fn save_presence(&self, presence: PresenceData) -> Result<PresenceData> {
        // Check if presence already exists
        match self.get_presence(&presence.user_id).await? {
            Some(existing) => {
                // Update existing presence
                let mut updated = presence;
                updated.id = existing.id;
                self.dao.update(&updated).await
            },
            None => {
                // Create new presence
                self.dao.create(&presence).await
            },
        }
    }

    /// Delete presence for a user
    pub async fn delete_presence(&self, user_id: &str) -> Result<Option<PresenceData>> {
        match self.get_presence(user_id).await? {
            Some(presence) => {
                match presence.id {
                    Some(id) => self.dao.delete(id).await,
                    None => Ok(None),
                }
            },
            None => Ok(None),
        }
    }

    /// Get whether batch operations are supported
    pub fn supports_batch_operations(&self) -> bool {
        // Not implemented yet
        false
    }

    /// Get presence for multiple users
    pub async fn get_presence_batch(&self, user_ids: &[String]) -> Result<Vec<PresenceData>> {
        let mut results = Vec::new();
        for user_id in user_ids {
            if let Some(presence) = self.get_presence(user_id).await? {
                results.push(presence);
            }
        }
        Ok(results)
    }

    /// Save presence in a transaction
    pub async fn save_presence_tx(
        &self,
        _tx: &crate::db::client::TransactionStream,
        user_id: &str,
        data: Value,
    ) -> Result<()> {
        // This is a placeholder implementation - would need proper transaction support
        let presence = PresenceData {
            id: None,
            user_id: user_id.to_string(),
            presence: "online".to_string(), // Default value
            last_active_ago: None,
            currently_active: None,
            status_msg: None,
            data,
            updated_at: chrono::Utc::now(),
        };

        self.save_presence(presence).await?;
        Ok(())
    }

    /// Delete presence in a transaction
    pub async fn delete_presence_tx(
        &self,
        _tx: &crate::db::client::TransactionStream,
        user_id: &str,
    ) -> Result<()> {
        // This is a placeholder implementation - would need proper transaction support
        self.delete_presence(user_id).await?;
        Ok(())
    }

    /// Get presence directly for a user
    pub fn get_presence_directly(&self, user_id: &str) -> MatrixFuture<Option<PresenceData>> {
        let dao = self.dao.clone();
        let user_id = user_id.to_string();

        MatrixFuture::spawn(async move {
            let presences: Vec<PresenceData> = dao
                .query_with_params::<Vec<PresenceData>>(
                    "SELECT * FROM presence WHERE user_id = $user LIMIT 1",
                    json!({ "user": user_id }),
                )
                .await?;

            Ok(presences.into_iter().next())
        })
    }

    /// Get presence event for a user
    pub fn get_presence_event(&self, user_id: &str) -> MatrixFuture<Option<Value>> {
        let dao = self.dao.clone();
        let user_id = user_id.to_string();

        MatrixFuture::spawn(async move {
            let presences: Vec<PresenceData> = dao
                .query_with_params::<Vec<PresenceData>>(
                    "SELECT * FROM presence WHERE user_id = $user LIMIT 1",
                    json!({ "user": user_id }),
                )
                .await?;

            if let Some(presence) = presences.first() {
                Ok(Some(presence.data.clone()))
            } else {
                Ok(None)
            }
        })
    }

    /// Save presence event
    pub fn save_presence_event(&self, user_id: &str, event: Value) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let user_id = user_id.to_string();
        let event = event.clone();

        MatrixFuture::spawn(async move {
            let now = Utc::now();

            // Try to update if exists
            let updated: Vec<PresenceData> = dao
                .query_with_params::<Vec<PresenceData>>(
                    "UPDATE presence SET event = $event, updated_at = $now WHERE user_id = $user",
                    json!({ "user": user_id, "event": event, "now": now }),
                )
                .await?;

            // If not updated, create new
            if updated.is_empty() {
                let presence = PresenceData { id: None, user_id, event, updated_at: now };

                let mut presence = presence;
                dao.create(&mut presence).await?;
            }

            Ok(())
        })
    }
}
