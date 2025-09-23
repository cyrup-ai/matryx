use crate::repository::error::RepositoryError;
use futures_util::StreamExt;
use matryx_entity::types::UserPresenceUpdate;
use surrealdb::{Surreal, engine::any::Any};

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
}
