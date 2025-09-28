use crate::repository::error::RepositoryError;
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

pub struct ProfileRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ProfileRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Get user profile by user ID
    pub async fn get_user_profile(
        &self,
        user_id: &str,
    ) -> Result<Option<UserProfile>, RepositoryError> {
        let query = "SELECT user_id, display_name, avatar_url FROM user WHERE user_id = $user_id";
        let mut response = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let profiles: Vec<UserProfile> = response.take(0)?;
        Ok(profiles.into_iter().next())
    }

    /// Update user display name
    pub async fn update_display_name(
        &self,
        user_id: &str,
        display_name: Option<String>,
    ) -> Result<(), RepositoryError> {
        let query = "UPDATE user SET display_name = $display_name WHERE user_id = $user_id";
        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("display_name", display_name.map(|s| s.to_string())))
            .await?;

        Ok(())
    }

    /// Update user avatar URL
    pub async fn update_avatar_url(
        &self,
        user_id: &str,
        avatar_url: Option<String>,
    ) -> Result<(), RepositoryError> {
        let query = "UPDATE user SET avatar_url = $avatar_url WHERE user_id = $user_id";
        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("avatar_url", avatar_url))
            .await?;

        Ok(())
    }

    /// Get user display name only
    pub async fn get_display_name(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT display_name FROM user WHERE user_id = $user_id";
        let mut response = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let result: Vec<serde_json::Value> = response.take(0)?;
        if let Some(row) = result.first() {
            let display_name = row
                .get("display_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            return Ok(display_name);
        }

        Ok(None)
    }

    /// Get user avatar URL only
    pub async fn get_avatar_url(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT avatar_url FROM user WHERE user_id = $user_id";
        let mut response = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let result: Vec<serde_json::Value> = response.take(0)?;
        if let Some(row) = result.first() {
            let avatar_url = row
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            return Ok(avatar_url);
        }

        Ok(None)
    }

    /// Check if user exists
    pub async fn user_exists(&self, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT count() FROM user WHERE user_id = $user_id GROUP ALL";
        let mut response = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let count: Option<i64> = response.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Get user profile with validation
    pub async fn get_validated_profile(
        &self,
        user_id: &str,
    ) -> Result<UserProfile, RepositoryError> {
        match self.get_user_profile(user_id).await? {
            Some(profile) => Ok(profile),
            None => Err(RepositoryError::NotFound {
                entity_type: "User".to_string(),
                id: user_id.to_string(),
            }),
        }
    }

    /// Update user profile (both display name and avatar URL)
    pub async fn update_profile(
        &self,
        user_id: &str,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<(), RepositoryError> {
        let query = "UPDATE user SET display_name = $display_name, avatar_url = $avatar_url WHERE user_id = $user_id";
        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("display_name", display_name))
            .bind(("avatar_url", avatar_url))
            .await?;

        Ok(())
    }
}