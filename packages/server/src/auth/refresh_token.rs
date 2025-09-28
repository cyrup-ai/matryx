use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::Connection;
use tracing::{error, info};
use uuid::Uuid;

use crate::auth::{MatrixAuthError, MatrixSessionService};
use matryx_surrealdb::repository::auth::AuthRepository;

/// Re-export the extended refresh token from repository
pub use matryx_surrealdb::repository::auth::ExtendedRefreshToken as RefreshToken;

/// Token pair containing access and refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub device_id: String,
}

/// Refresh token request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// Refresh token response
#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// Service for managing refresh tokens with automatic rotation
pub struct RefreshTokenService<C: Connection> {
    auth_repo: AuthRepository<C>,
    session_service: Arc<MatrixSessionService<C>>,
    access_token_lifetime: Duration,
    refresh_token_lifetime: Duration,
    max_rotation_count: i32,
}

impl<C: Connection> RefreshTokenService<C> {
    pub fn new(
        auth_repo: AuthRepository<C>,
        session_service: Arc<MatrixSessionService<C>>,
    ) -> Self {
        Self {
            auth_repo,
            session_service,
            access_token_lifetime: Duration::hours(1), // 1 hour
            refresh_token_lifetime: Duration::days(30), // 30 days
            max_rotation_count: 100,                   // Maximum number of rotations
        }
    }

    /// Create initial token pair for user login
    pub async fn create_initial_tokens(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<TokenPair, MatrixAuthError> {
        let access_token = format!("syt_{}", Uuid::new_v4());
        let refresh_token = format!("syr_{}", Uuid::new_v4());
        let now = Utc::now();

        // Store refresh token in database
        let refresh_record = RefreshToken {
            token: refresh_token.clone(),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            access_token: access_token.clone(),
            created_at: now,
            expires_at: now + self.refresh_token_lifetime,
            used: false,
            revoked: false,
            rotation_count: 0,
            parent_token: None,
        };

        self.auth_repo
            .store_extended_refresh_token(&refresh_record)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to store refresh token: {}", e))
            })?;

        // Store access token in session service
        let _matrix_token = self
            .session_service
            .create_user_session(user_id, device_id, &access_token, Some(&refresh_token))
            .await?;

        info!("Created initial token pair for user: {} device: {}", user_id, device_id);

        Ok(TokenPair {
            access_token,
            refresh_token,
            expires_in: self.access_token_lifetime.num_seconds(),
            device_id: device_id.to_string(),
        })
    }

    /// Refresh access token using refresh token with automatic rotation
    pub async fn refresh_tokens(
        &self,
        refresh_token: &str,
    ) -> Result<RefreshTokenResponse, MatrixAuthError> {
        // Validate and retrieve refresh token
        let old_refresh = self
            .auth_repo
            .validate_extended_refresh_token(refresh_token)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to validate refresh token: {}", e))
            })?
            .ok_or(MatrixAuthError::UnknownToken)?;

        // Check rotation limits
        if old_refresh.rotation_count >= self.max_rotation_count {
            error!("Refresh token rotation limit exceeded for user: {}", old_refresh.user_id);
            self.revoke_refresh_token(refresh_token).await?;
            return Err(MatrixAuthError::SessionExpired);
        }

        // Generate new tokens
        let new_access_token = format!("syt_{}", Uuid::new_v4());
        let new_refresh_token = format!("syr_{}", Uuid::new_v4());
        let now = Utc::now();

        // Create new refresh token record
        let new_refresh_record = RefreshToken {
            token: new_refresh_token.clone(),
            user_id: old_refresh.user_id.clone(),
            device_id: old_refresh.device_id.clone(),
            access_token: new_access_token.clone(),
            created_at: now,
            expires_at: now + self.refresh_token_lifetime,
            used: false,
            revoked: false,
            rotation_count: old_refresh.rotation_count + 1,
            parent_token: Some(old_refresh.token.clone()),
        };

        // Store new refresh token
        self.auth_repo
            .store_extended_refresh_token(&new_refresh_record)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to store new refresh token: {}", e))
            })?;

        // Mark old refresh token as used
        self.auth_repo
            .mark_refresh_token_used(&old_refresh.token)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to mark token as used: {}", e))
            })?;

        // Create new session with rotated tokens
        let _matrix_token = self
            .session_service
            .create_user_session(
                &old_refresh.user_id,
                &old_refresh.device_id,
                &new_access_token,
                Some(&new_refresh_token),
            )
            .await?;

        info!(
            "Refreshed tokens for user: {} device: {} (rotation: {})",
            old_refresh.user_id, old_refresh.device_id, new_refresh_record.rotation_count
        );

        Ok(RefreshTokenResponse {
            access_token: new_access_token,
            refresh_token: new_refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.access_token_lifetime.num_seconds(),
        })
    }

    /// Revoke refresh token and all tokens in its rotation chain
    pub async fn revoke_refresh_token(&self, refresh_token: &str) -> Result<(), MatrixAuthError> {
        // Get the refresh token to find its chain
        let token_record = self
            .auth_repo
            .get_extended_refresh_token(refresh_token)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to get refresh token: {}", e))
            })?
            .ok_or(MatrixAuthError::UnknownToken)?;

        // Revoke the token and its entire rotation chain
        self.revoke_token_chain(&token_record).await?;

        info!("Revoked refresh token and chain for user: {}", token_record.user_id);
        Ok(())
    }

    /// Revoke all refresh tokens for a user
    pub async fn revoke_all_user_tokens(&self, user_id: &str) -> Result<(), MatrixAuthError> {
        self.auth_repo.revoke_all_user_refresh_tokens(user_id).await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to revoke user tokens: {}", e))
        })?;

        info!("Revoked all refresh tokens for user: {}", user_id);
        Ok(())
    }

    /// Revoke all refresh tokens for a device
    pub async fn revoke_device_tokens(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), MatrixAuthError> {
        self.auth_repo
            .revoke_device_refresh_tokens(user_id, device_id)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to revoke device tokens: {}", e))
            })?;

        info!("Revoked all refresh tokens for user: {} device: {}", user_id, device_id);
        Ok(())
    }

    /// Clean up expired and used refresh tokens
    pub async fn cleanup_expired_tokens(&self) -> Result<u64, MatrixAuthError> {
        let count =
            self.auth_repo
                .cleanup_expired_refresh_tokens_extended()
                .await
                .map_err(|e| {
                    MatrixAuthError::DatabaseError(format!("Failed to cleanup tokens: {}", e))
                })?;

        if count > 0 {
            info!("Cleaned up {} expired refresh tokens", count);
        }

        Ok(count)
    }

    /// Get refresh token statistics for monitoring
    pub async fn get_token_stats(&self) -> Result<TokenStats, MatrixAuthError> {
        let db_stats = self.auth_repo.get_refresh_token_stats().await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to get token stats: {}", e))
        })?;

        // Convert from surrealdb::TokenStats to local TokenStats
        Ok(TokenStats {
            total_tokens: db_stats.total_tokens,
            revoked_count: db_stats.revoked_count,
            used_count: db_stats.used_count,
            expired_count: db_stats.expired_count,
        })
    }

    /// Revoke entire token rotation chain for security
    async fn revoke_token_chain(&self, token: &RefreshToken) -> Result<(), MatrixAuthError> {
        // Find the root token of the chain
        let mut current_token = token.clone();
        while let Some(ref parent) = current_token.parent_token {
            match self.auth_repo.get_extended_refresh_token(parent).await {
                Ok(Some(parent_token)) => current_token = parent_token,
                _ => break, // Parent not found, we're at the root
            }
        }

        // Revoke all tokens in the chain starting from root
        self.auth_repo
            .revoke_token_chain_from_root(&current_token.token)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to revoke token chain: {}", e))
            })?;

        Ok(())
    }
}

/// Token statistics for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenStats {
    pub total_tokens: u64,
    pub revoked_count: u64,
    pub used_count: u64,
    pub expired_count: u64,
}

/// Configuration for refresh token service
#[derive(Debug, Clone)]
pub struct RefreshTokenConfig {
    pub access_token_lifetime_hours: i64,
    pub refresh_token_lifetime_days: i64,
    pub max_rotation_count: i32,
    pub cleanup_interval_hours: u64,
}

impl RefreshTokenConfig {
    pub fn from_env() -> Self {
        Self {
            access_token_lifetime_hours: std::env::var("ACCESS_TOKEN_LIFETIME_HOURS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            refresh_token_lifetime_days: std::env::var("REFRESH_TOKEN_LIFETIME_DAYS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            max_rotation_count: std::env::var("MAX_TOKEN_ROTATION_COUNT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
            cleanup_interval_hours: std::env::var("TOKEN_CLEANUP_INTERVAL_HOURS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(24),
        }
    }
}
