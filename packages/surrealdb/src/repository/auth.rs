use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_revoked: bool,
}

/// Extended refresh token record for token rotation support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedRefreshToken {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub revoked: bool,
    pub rotation_count: i32,
    pub parent_token: Option<String>,
}

/// Token statistics for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenStats {
    pub total_tokens: u64,
    pub revoked_count: u64,
    pub used_count: u64,
    pub expired_count: u64,
}

pub struct AuthRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> AuthRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Validate user credentials against stored password hash
    pub async fn validate_user_credentials(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT password_hash FROM user 
            WHERE user_id = $user_id AND is_active = true
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let users: Vec<serde_json::Value> = result.take(0)?;

        if let Some(user) = users.first()
            && let Some(stored_hash) = user.get("password_hash").and_then(|v| v.as_str()) {
            return Ok(stored_hash == password_hash);
        }

        Ok(false)
    }

    /// Check if user has admin privileges
    pub async fn is_user_admin(&self, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT is_admin FROM user 
            WHERE user_id = $user_id AND is_active = true
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let users: Vec<serde_json::Value> = result.take(0)?;

        if let Some(user) = users.first()
            && let Some(is_admin) = user.get("is_admin").and_then(|v| v.as_bool()) {
            return Ok(is_admin);
        }

        Ok(false)
    }

    /// Check if user account is active
    pub async fn is_user_active(&self, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT is_active FROM user 
            WHERE user_id = $user_id
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let users: Vec<serde_json::Value> = result.take(0)?;

        if let Some(user) = users.first()
            && let Some(is_active) = user.get("is_active").and_then(|v| v.as_bool()) {
            return Ok(is_active);
        }

        Ok(false)
    }

    /// Check if user is a member of a specific room
    pub async fn check_user_membership(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() FROM membership 
            WHERE user_id = $user_id AND room_id = $room_id 
            AND membership = 'join'
            GROUP ALL
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Validate device ownership
    pub async fn validate_device(
        &self,
        device_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() FROM device 
            WHERE device_id = $device_id AND user_id = $user_id
            GROUP ALL
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("device_id", device_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Create a new refresh token
    pub async fn create_refresh_token(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<RefreshToken, RepositoryError> {
        let token = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::days(30); // 30 day expiry

        let refresh_token = RefreshToken {
            token: token.clone(),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            created_at: now,
            expires_at,
            is_revoked: false,
        };

        let created: Option<RefreshToken> = self
            .db
            .create(("refresh_token", &token))
            .content(refresh_token.clone())
            .await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create refresh token"))
        })
    }

    /// Validate and retrieve refresh token
    pub async fn validate_refresh_token(
        &self,
        token: &str,
    ) -> Result<Option<RefreshToken>, RepositoryError> {
        let refresh_token: Option<RefreshToken> = self.db.select(("refresh_token", token)).await?;

        if let Some(token_data) = refresh_token {
            // Check if token is expired or revoked
            if token_data.is_revoked || token_data.expires_at < Utc::now() {
                return Ok(None);
            }
            return Ok(Some(token_data));
        }

        Ok(None)
    }

    /// Revoke a refresh token
    pub async fn revoke_refresh_token(&self, token: &str) -> Result<(), RepositoryError> {
        let query = "
            UPDATE refresh_token SET is_revoked = true, revoked_at = $revoked_at
            WHERE token = $token
        ";

        self.db
            .query(query)
            .bind(("token", token.to_string()))
            .bind(("revoked_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Get user by ID with full details
    pub async fn get_user(
        &self,
        user_id: &str,
    ) -> Result<Option<serde_json::Value>, RepositoryError> {
        let query = "
            SELECT * FROM user 
            WHERE user_id = $user_id
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let users: Vec<serde_json::Value> = result.take(0)?;
        Ok(users.into_iter().next())
    }

    /// Update user last seen timestamp
    pub async fn update_user_last_seen(&self, user_id: &str) -> Result<(), RepositoryError> {
        let query = "
            UPDATE user SET last_seen_at = $last_seen_at
            WHERE user_id = $user_id
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("last_seen_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Cleanup expired refresh tokens
    pub async fn cleanup_expired_refresh_tokens(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "
            DELETE refresh_token 
            WHERE expires_at < $cutoff OR is_revoked = true
        ";

        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;

        let deleted: Option<Vec<serde_json::Value>> = result.take(0)?;
        Ok(deleted.map(|v| v.len() as u64).unwrap_or(0))
    }

    // Extended refresh token methods for token rotation support

    /// Store extended refresh token with rotation support
    pub async fn store_extended_refresh_token(
        &self,
        refresh_token: &ExtendedRefreshToken,
    ) -> Result<(), RepositoryError> {
        let _: Option<ExtendedRefreshToken> = self
            .db
            .create(("refresh_tokens", &refresh_token.token))
            .content(refresh_token.clone())
            .await?;
        Ok(())
    }

    /// Get extended refresh token by token
    pub async fn get_extended_refresh_token(
        &self,
        token: &str,
    ) -> Result<Option<ExtendedRefreshToken>, RepositoryError> {
        let refresh_token: Option<ExtendedRefreshToken> =
            self.db.select(("refresh_tokens", token)).await?;
        Ok(refresh_token)
    }

    /// Validate extended refresh token and return if valid
    pub async fn validate_extended_refresh_token(
        &self,
        token: &str,
    ) -> Result<Option<ExtendedRefreshToken>, RepositoryError> {
        let refresh_token = self.get_extended_refresh_token(token).await?;

        if let Some(token_data) = refresh_token {
            // Check if token is revoked
            if token_data.revoked {
                return Ok(None);
            }

            // Check if token is already used
            if token_data.used {
                return Ok(None);
            }

            // Check if token has expired
            if Utc::now() > token_data.expires_at {
                return Ok(None);
            }

            return Ok(Some(token_data));
        }

        Ok(None)
    }

    /// Mark refresh token as used
    pub async fn mark_refresh_token_used(&self, token: &str) -> Result<(), RepositoryError> {
        let query = "
            UPDATE refresh_tokens 
            SET used = true, updated_at = datetime::now()
            WHERE id = $token
        ";

        self.db.query(query).bind(("token", token.to_string())).await?;

        Ok(())
    }

    /// Revoke all refresh tokens for a user
    pub async fn revoke_all_user_refresh_tokens(
        &self,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE refresh_tokens 
            SET revoked = true, updated_at = datetime::now()
            WHERE user_id = $user_id AND revoked = false
        ";

        self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        Ok(())
    }

    /// Revoke all refresh tokens for a device
    pub async fn revoke_device_refresh_tokens(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE refresh_tokens 
            SET revoked = true, updated_at = datetime::now()
            WHERE user_id = $user_id AND device_id = $device_id AND revoked = false
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        Ok(())
    }

    /// Revoke token chain starting from root token
    pub async fn revoke_token_chain_from_root(
        &self,
        root_token: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE refresh_tokens 
            SET revoked = true, updated_at = datetime::now()
            WHERE token = $root_token OR parent_token = $root_token
        ";

        self.db.query(query).bind(("root_token", root_token.to_string())).await?;

        Ok(())
    }

    /// Clean up expired and used refresh tokens
    pub async fn cleanup_expired_refresh_tokens_extended(&self) -> Result<u64, RepositoryError> {
        let query = "
            DELETE FROM refresh_tokens 
            WHERE expires_at < datetime::now() OR (used = true AND created_at < datetime::sub(datetime::now(), duration('7d')))
        ";

        let mut response = self.db.query(query).await?;

        // Get the count of deleted records
        let deleted_count: Option<u64> = response.take(0).unwrap_or(Some(0));
        let count = deleted_count.unwrap_or(0);

        Ok(count)
    }

    /// Get refresh token statistics for monitoring
    pub async fn get_refresh_token_stats(&self) -> Result<TokenStats, RepositoryError> {
        let query = "
            SELECT 
                count() as total_tokens,
                count(revoked = true) as revoked_count,
                count(used = true) as used_count,
                count(expires_at < datetime::now()) as expired_count
            FROM refresh_tokens
        ";

        let mut response = self.db.query(query).await?;

        let stats: Option<TokenStats> = response.take(0)?;

        stats.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "TokenStats".to_string(),
                id: "default".to_string(),
            }
        })
    }
}
