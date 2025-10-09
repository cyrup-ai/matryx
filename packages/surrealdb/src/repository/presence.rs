use crate::repository::error::RepositoryError;
use futures_util::StreamExt;
use matryx_entity::types::UserPresenceUpdate;
use surrealdb::{Surreal, engine::any::Any};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// TASK14 SUBTASK 3: Add supporting types for presence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEvent {
    pub user_id: String,
    pub presence: String,
    pub status_msg: Option<String>,
    pub last_active_ago: Option<u64>,
    pub currently_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PresenceState {
    Online,
    Offline,
    Unavailable,
}

#[derive(Clone)]
pub struct PresenceRepository {
    db: Surreal<Any>,
}

impl PresenceRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Create or update user presence
    pub async fn update_user_presence(
        &self,
        user_id: &str,
        presence: &UserPresenceUpdate,
    ) -> Result<UserPresenceUpdate, RepositoryError> {
        let presence_clone = presence.clone();
        let updated: Option<UserPresenceUpdate> =
            self.db.update(("user_presence", user_id)).content(presence_clone).await?;

        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update user presence"))
        })
    }

    /// Get current user presence
    pub async fn get_user_presence(
        &self,
        user_id: &str,
    ) -> Result<Option<UserPresenceUpdate>, RepositoryError> {
        let presence: Option<UserPresenceUpdate> =
            self.db.select(("user_presence", user_id)).await?;
        Ok(presence)
    }

    /// Subscribe to user presence changes using SurrealDB LiveQuery
    /// Returns a stream of notifications for presence changes for the specified user
    pub async fn subscribe_to_user_presence(
        &self,
        user_id: &str,
    ) -> Result<
        impl futures_util::Stream<Item = Result<UserPresenceUpdate, RepositoryError>>,
        RepositoryError,
    > {
        // Create SurrealDB LiveQuery for user presence for specific user
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM user_presence WHERE user_id = $user_id")
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to presence stream
        let presence_stream = stream
            .stream::<surrealdb::Notification<UserPresenceUpdate>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<UserPresenceUpdate, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted presence, return the data for proper handling
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(presence_stream)
    }

    /// Set user offline
    pub async fn set_user_offline(&self, user_id: &str) -> Result<(), RepositoryError> {
        let offline_presence = UserPresenceUpdate {
            user_id: user_id.to_string(),
            presence: "offline".to_string(),
            last_active_ago: None,
            status_msg: None,
            currently_active: Some(false),
        };

        self.update_user_presence(user_id, &offline_presence).await?;
        Ok(())
    }

    /// Set user online
    pub async fn set_user_online(
        &self,
        user_id: &str,
        status_msg: Option<String>,
    ) -> Result<(), RepositoryError> {
        let online_presence = UserPresenceUpdate {
            user_id: user_id.to_string(),
            presence: "online".to_string(),
            last_active_ago: Some(0),
            status_msg,
            currently_active: Some(true),
        };

        self.update_user_presence(user_id, &online_presence).await?;
        Ok(())
    }

    /// Set user unavailable
    pub async fn set_user_unavailable(
        &self,
        user_id: &str,
        status_msg: Option<String>,
    ) -> Result<(), RepositoryError> {
        let unavailable_presence = UserPresenceUpdate {
            user_id: user_id.to_string(),
            presence: "unavailable".to_string(),
            last_active_ago: None,
            status_msg,
            currently_active: Some(false),
        };

        self.update_user_presence(user_id, &unavailable_presence).await?;
        Ok(())
    }

    /// Get presence for multiple users
    pub async fn get_multiple_user_presence(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<UserPresenceUpdate>, RepositoryError> {
        let query = "SELECT * FROM user_presence WHERE user_id IN $user_ids";
        let mut result = self.db.query(query).bind(("user_ids", user_ids.to_vec())).await?;
        let presences: Vec<UserPresenceUpdate> = result.take(0)?;
        Ok(presences)
    }

    // TASK14 SUBTASK 3: Add missing presence methods

    /// Get user presence events since a specific time
    pub async fn get_user_presence_events(&self, user_id: &str, since: Option<DateTime<Utc>>) -> Result<Vec<PresenceEvent>, RepositoryError> {
        let mut query = String::from("SELECT * FROM presence_events WHERE user_id = $user_id");
        let mut query_builder = self.db.query(&query).bind(("user_id", user_id.to_string()));

        if let Some(since_time) = since {
            query.push_str(" AND updated_at > $since");
            query_builder = query_builder.bind(("since", since_time));
        }

        query.push_str(" ORDER BY updated_at DESC LIMIT 100");
        
        let mut response = query_builder.await.map_err(RepositoryError::Database)?;
        let events: Vec<PresenceEvent> = response.take(0).map_err(RepositoryError::Database)?;
        Ok(events)
    }

    /// Get presence events for multiple users since a specific time
    pub async fn get_presence_events_for_users(&self, user_ids: &[String], since: Option<DateTime<Utc>>) -> Result<Vec<PresenceEvent>, RepositoryError> {
        let mut query = String::from("SELECT * FROM presence_events WHERE user_id IN $user_ids");
        let mut query_builder = self.db.query(&query).bind(("user_ids", user_ids.to_vec()));

        if let Some(since_time) = since {
            query.push_str(" AND updated_at > $since");
            query_builder = query_builder.bind(("since", since_time));
        }

        query.push_str(" ORDER BY updated_at DESC");
        
        let mut response = query_builder.await.map_err(RepositoryError::Database)?;
        let events: Vec<PresenceEvent> = response.take(0).map_err(RepositoryError::Database)?;
        Ok(events)
    }

    /// Update user presence with state and status message  
    pub async fn update_user_presence_state(&self, user_id: &str, presence: PresenceState, status_msg: Option<&str>) -> Result<(), RepositoryError> {
        let presence_str = match presence {
            PresenceState::Online => "online",
            PresenceState::Offline => "offline",
            PresenceState::Unavailable => "unavailable",
        };

        let query = r#"
            UPSERT presence_events:⟨$user_id⟩ CONTENT {
                user_id: $user_id,
                presence: $presence,
                status_msg: $status_msg,
                last_active_ago: $last_active_ago,
                currently_active: $currently_active,
                updated_at: time::now()
            }
        "#;

        let currently_active = matches!(presence, PresenceState::Online);
        let last_active_ago = if currently_active { Some(0) } else { None };

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("presence", presence_str.to_string()))
            .bind(("status_msg", status_msg.map(|s| s.to_string())))
            .bind(("last_active_ago", last_active_ago))
            .bind(("currently_active", currently_active))
            .await
            .map_err(RepositoryError::Database)?;

        Ok(())
    }

    /// Get user's last active time
    pub async fn get_user_last_active(&self, user_id: &str) -> Result<Option<DateTime<Utc>>, RepositoryError> {
        let query = "SELECT updated_at FROM presence_events WHERE user_id = $user_id ORDER BY updated_at DESC LIMIT 1";
        
        let mut response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        let times: Vec<DateTime<Utc>> = response.take(0).map_err(RepositoryError::Database)?;
        Ok(times.into_iter().next())
    }

    /// Cleanup old presence events
    pub async fn cleanup_old_presence_events(&self, cutoff: DateTime<Utc>) -> Result<u64, RepositoryError> {
        // Count records to be deleted first
        let count_query = "SELECT count() as count FROM presence_events WHERE updated_at < $cutoff";
        let mut count_response = self.db
            .query(count_query)
            .bind(("cutoff", cutoff))
            .await
            .map_err(RepositoryError::Database)?;
        
        #[derive(serde::Deserialize)]
        struct CountResult {
            count: Option<i64>,
        }
        
        let count_results: Vec<CountResult> = count_response.take(0).map_err(RepositoryError::Database)?;
        let count = count_results
            .into_iter()
            .next()
            .and_then(|r| r.count)
            .unwrap_or(0) as u64;

        // Delete old presence events
        let delete_query = "DELETE presence_events WHERE updated_at < $cutoff";
        let mut _delete_response = self.db
            .query(delete_query)
            .bind(("cutoff", cutoff))
            .await
            .map_err(RepositoryError::Database)?;

        Ok(count)
    }


}
