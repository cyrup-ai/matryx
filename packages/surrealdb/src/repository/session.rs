use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use matryx_entity::types::Session;
use serde::{Deserialize, Serialize};
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSigningKey {
    pub key_id: String,
    pub key_data: String,
    pub algorithm: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Clone)]
pub struct SessionRepository {
    db: Surreal<Any>,
}

impl SessionRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, session: &Session) -> Result<Session, RepositoryError> {
        let session_clone = session.clone();
        let created: Option<Session> = self
            .db
            .create(("session", &session.session_id))
            .content(session_clone)
            .await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create session"))
        })
    }

    pub async fn get_by_id(&self, session_id: &str) -> Result<Option<Session>, RepositoryError> {
        let session: Option<Session> = self.db.select(("session", session_id)).await?;
        Ok(session)
    }

    pub async fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<Session>, RepositoryError> {
        let query =
            "SELECT * FROM session WHERE access_token = $token AND is_active = true LIMIT 1";
        let mut result = self.db.query(query).bind(("token", access_token.to_string())).await?;
        let sessions: Vec<Session> = result.take(0)?;
        Ok(sessions.into_iter().next())
    }

    pub async fn update(&self, session: &Session) -> Result<Session, RepositoryError> {
        let session_clone = session.clone();
        let updated: Option<Session> = self
            .db
            .update(("session", &session.session_id))
            .content(session_clone)
            .await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update session"))
        })
    }

    pub async fn delete(&self, session_id: &str) -> Result<(), RepositoryError> {
        let _: Option<Session> = self.db.delete(("session", session_id)).await?;
        Ok(())
    }

    pub async fn deactivate(&self, session_id: &str) -> Result<(), RepositoryError> {
        let query = "UPDATE session SET is_active = false WHERE session_id = $session_id";
        self.db.query(query).bind(("session_id", session_id.to_string())).await?;
        Ok(())
    }

    pub async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<Session>, RepositoryError> {
        let query = "SELECT * FROM session WHERE user_id = $user_id AND is_active = true";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let sessions: Vec<Session> = result.take(0)?;
        Ok(sessions)
    }

    pub async fn deactivate_all_user_sessions(&self, user_id: &str) -> Result<(), RepositoryError> {
        let query = "UPDATE session SET is_active = false WHERE user_id = $user_id";
        self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        Ok(())
    }

    pub async fn invalidate_token(&self, access_token: &str) -> Result<(), RepositoryError> {
        let query = "UPDATE session SET is_active = false WHERE access_token = $token";
        self.db.query(query).bind(("token", access_token.to_string())).await?;
        Ok(())
    }

    pub async fn invalidate_all_tokens(&self, user_id: &str) -> Result<(), RepositoryError> {
        self.deactivate_all_user_sessions(user_id).await
    }

    pub async fn delete_by_user(&self, user_id: &str) -> Result<(), RepositoryError> {
        let query = "DELETE FROM session WHERE user_id = $user_id";
        self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        Ok(())
    }

    /// Create a server signing key
    pub async fn create_server_signing_key(
        &self,
        key_id: &str,
        key_data: &ServerSigningKey,
    ) -> Result<(), RepositoryError> {
        let _: Option<ServerSigningKey> = self
            .db
            .create(("server_signing_key", key_id))
            .content(key_data.clone())
            .await?;
        Ok(())
    }

    /// Get server signing key by ID
    pub async fn get_server_signing_key(
        &self,
        key_id: &str,
    ) -> Result<Option<ServerSigningKey>, RepositoryError> {
        let key: Option<ServerSigningKey> = self.db.select(("server_signing_key", key_id)).await?;
        Ok(key)
    }

    /// List all server signing keys
    pub async fn list_server_signing_keys(&self) -> Result<Vec<ServerSigningKey>, RepositoryError> {
        let query = "SELECT * FROM server_signing_key ORDER BY created_at DESC";
        let mut result = self.db.query(query).await?;
        let keys: Vec<ServerSigningKey> = result.take(0)?;
        Ok(keys)
    }

    /// Rotate signing keys (deactivate old, activate new)
    pub async fn rotate_signing_keys(
        &self,
        old_key_id: &str,
        new_key: &ServerSigningKey,
    ) -> Result<(), RepositoryError> {
        // Deactivate old key
        let query = "UPDATE server_signing_key SET is_active = false WHERE key_id = $key_id";
        self.db.query(query).bind(("key_id", old_key_id.to_string())).await?;

        // Create new key
        self.create_server_signing_key(&new_key.key_id, new_key).await?;

        Ok(())
    }

    /// Cleanup expired sessions
    pub async fn cleanup_expired_sessions(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "
            DELETE session 
            WHERE expires_at < $cutoff OR (is_active = false AND updated_at < $cutoff)
        ";

        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;

        let deleted: Option<Vec<serde_json::Value>> = result.take(0)?;
        Ok(deleted.map(|v| v.len() as u64).unwrap_or(0))
    }

    /// Revoke user sessions except specified one
    pub async fn revoke_user_sessions(
        &self,
        user_id: &str,
        except_session: Option<&str>,
    ) -> Result<u64, RepositoryError> {
        let query = if except_session.is_some() {
            "UPDATE session SET is_active = false WHERE user_id = $user_id AND session_id != $except_id AND is_active = true"
        } else {
            "UPDATE session SET is_active = false WHERE user_id = $user_id AND is_active = true"
        };

        let mut result = if let Some(except_id) = except_session {
            self.db
                .query(query)
                .bind(("user_id", user_id.to_string()))
                .bind(("except_id", except_id.to_string()))
                .await?
        } else {
            self.db.query(query).bind(("user_id", user_id.to_string())).await?
        };

        let updated: Option<Vec<serde_json::Value>> = result.take(0)?;
        Ok(updated.map(|v| v.len() as u64).unwrap_or(0))
    }

    /// Get active server signing key
    pub async fn get_active_signing_key(
        &self,
    ) -> Result<Option<ServerSigningKey>, RepositoryError> {
        let query = "
            SELECT * FROM server_signing_key 
            WHERE is_active = true 
            ORDER BY created_at DESC 
            LIMIT 1
        ";
        let mut result = self.db.query(query).await?;
        let keys: Vec<ServerSigningKey> = result.take(0)?;
        Ok(keys.into_iter().next())
    }

    /// Update session last activity
    pub async fn update_session_activity(&self, session_id: &str) -> Result<(), RepositoryError> {
        let query = "
            UPDATE session 
            SET last_activity = $last_activity 
            WHERE session_id = $session_id
        ";

        self.db
            .query(query)
            .bind(("session_id", session_id.to_string()))
            .bind(("last_activity", Utc::now()))
            .await?;

        Ok(())
    }

    /// Get session count for user
    pub async fn get_user_session_count(&self, user_id: &str) -> Result<u64, RepositoryError> {
        let query = "
            SELECT count() FROM session 
            WHERE user_id = $user_id AND is_active = true
            GROUP ALL
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) as u64)
    }

    /// Get user access token for opaque token validation
    pub async fn get_user_access_token(
        &self,
        token: &str,
    ) -> Result<Option<UserAccessToken>, RepositoryError> {
        let query = "
            SELECT user_id, device_id, expires_at
            FROM user_access_tokens
            WHERE token = $token AND (expires_at IS NULL OR expires_at > datetime::now())
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("token", token.to_string())).await?;

        let token_records: Vec<UserAccessToken> = result.take(0)?;
        Ok(token_records.into_iter().next())
    }

    /// Create user access token
    pub async fn create_user_access_token(
        &self,
        token_data: &UserAccessToken,
    ) -> Result<(), RepositoryError> {
        let _: Option<UserAccessToken> = self
            .db
            .create(("user_access_tokens", &token_data.token))
            .content(token_data.clone())
            .await?;
        Ok(())
    }

    /// Update user access token expiry
    pub async fn update_user_access_token_expiry(
        &self,
        token: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE user_access_tokens 
            SET expires_at = $expires_at 
            WHERE token = $token
        ";

        self.db
            .query(query)
            .bind(("token", token.to_string()))
            .bind(("expires_at", expires_at))
            .await?;

        Ok(())
    }

    /// Delete user access token
    pub async fn delete_user_access_token(&self, token: &str) -> Result<(), RepositoryError> {
        let _: Option<UserAccessToken> = self.db.delete(("user_access_tokens", token)).await?;
        Ok(())
    }

    /// Get all user access tokens for a user
    pub async fn get_user_access_tokens(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserAccessToken>, RepositoryError> {
        let query = "
            SELECT * FROM user_access_tokens 
            WHERE user_id = $user_id 
            ORDER BY created_at DESC
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let tokens: Vec<UserAccessToken> = result.take(0)?;
        Ok(tokens)
    }

    /// Clean up LiveQuery subscriptions for a user/device
    pub async fn cleanup_livequery_subscriptions(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            DELETE FROM livequery_subscriptions 
            WHERE user_id = $user_id AND device_id = $device_id
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        Ok(())
    }

    /// Revoke device refresh tokens
    pub async fn revoke_device_refresh_tokens(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE refresh_tokens 
            SET revoked = true, revoked_at = datetime::now()
            WHERE user_id = $user_id AND device_id = $device_id AND revoked = false
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        Ok(())
    }

    /// Get user access tokens for admin whois
    pub async fn get_user_access_tokens_for_admin(
        &self,
        user_id: &str,
    ) -> Result<Vec<(String, Option<String>, Option<i64>)>, RepositoryError> {
        let query = "
            SELECT device_id, last_used_ip, last_used_ts
            FROM user_access_tokens
            WHERE user_id = $user_id AND expires_at > time::now()
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let tokens: Vec<(String, Option<String>, Option<i64>)> = result.take(0)?;
        Ok(tokens)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccessToken {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}
