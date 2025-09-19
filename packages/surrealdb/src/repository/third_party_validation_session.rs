use async_trait::async_trait;
use chrono::Utc;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{debug, error, info, warn};

use crate::repository::error::RepositoryError;
use matryx_entity::types::third_party_validation_session::ThirdPartyValidationSession;

/// Repository for managing third-party validation sessions
/// 
/// Provides CRUD operations for 3PID validation sessions used in the Matrix
/// Client-Server API third-party identifier validation flow.
#[derive(Clone)]
pub struct ThirdPartyValidationSessionRepository {
    db: Surreal<Any>,
}

impl ThirdPartyValidationSessionRepository {
    /// Create a new repository instance
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}

#[async_trait]
pub trait ThirdPartyValidationSessionRepositoryTrait {
    /// Create a new validation session
    async fn create_session(&self, session: &ThirdPartyValidationSession) -> Result<(), RepositoryError>;
    
    /// Get session by session ID
    async fn get_session_by_id(&self, session_id: &str) -> Result<Option<ThirdPartyValidationSession>, RepositoryError>;
    
    /// Get session by session ID and client secret
    async fn get_session_by_id_and_secret(&self, session_id: &str, client_secret: &str) -> Result<Option<ThirdPartyValidationSession>, RepositoryError>;
    
    /// Update session verification status
    async fn mark_session_verified(&self, session_id: &str) -> Result<(), RepositoryError>;
    
    /// Update session attempt count
    async fn increment_session_attempts(&self, session_id: &str) -> Result<(), RepositoryError>;
    
    /// Associate session with user
    async fn associate_session_with_user(&self, session_id: &str, user_id: &str) -> Result<(), RepositoryError>;
    
    /// Delete session
    async fn delete_session(&self, session_id: &str) -> Result<(), RepositoryError>;
    
    /// Get sessions by address (for duplicate checking)
    async fn get_sessions_by_address(&self, medium: &str, address: &str) -> Result<Vec<ThirdPartyValidationSession>, RepositoryError>;
    
    /// Clean up expired sessions
    async fn cleanup_expired_sessions(&self) -> Result<u64, RepositoryError>;
    
    /// Get active sessions for user
    async fn get_active_sessions_for_user(&self, user_id: &str) -> Result<Vec<ThirdPartyValidationSession>, RepositoryError>;
}

#[async_trait]
impl ThirdPartyValidationSessionRepositoryTrait for ThirdPartyValidationSessionRepository {
    async fn create_session(&self, session: &ThirdPartyValidationSession) -> Result<(), RepositoryError> {
        let query = r#"
            CREATE third_party_sessions SET
                session_id = $session_id,
                client_secret = $client_secret,
                medium = $medium,
                address = $address,
                verification_token = $verification_token,
                verified = $verified,
                expires_at = $expires_at,
                user_id = $user_id,
                created_at = $created_at,
                validated_at = $validated_at,
                attempt_count = $attempt_count,
                max_attempts = $max_attempts
        "#;

        let result = self.db
            .query(query)
            .bind(("session_id", session.session_id.clone()))
            .bind(("client_secret", session.client_secret.clone()))
            .bind(("medium", session.medium.clone()))
            .bind(("address", session.address.clone()))
            .bind(("verification_token", session.verification_token.clone()))
            .bind(("verified", session.verified))
            .bind(("expires_at", session.expires_at))
            .bind(("user_id", session.user_id.clone()))
            .bind(("created_at", session.created_at))
            .bind(("validated_at", session.validated_at))
            .bind(("attempt_count", session.attempt_count))
            .bind(("max_attempts", session.max_attempts))
            .await?;

        debug!("Created 3PID validation session: {}", session.session_id);
        Ok(())
    }

    async fn get_session_by_id(&self, session_id: &str) -> Result<Option<ThirdPartyValidationSession>, RepositoryError> {
        let query = "SELECT * FROM third_party_sessions WHERE session_id = $session_id";
        
        let mut result = self.db
            .query(query)
            .bind(("session_id", session_id.to_string()))
            .await?;

        let sessions: Vec<ThirdPartyValidationSession> = result.take(0)?;
        
        Ok(sessions.into_iter().next())
    }

    async fn get_session_by_id_and_secret(&self, session_id: &str, client_secret: &str) -> Result<Option<ThirdPartyValidationSession>, RepositoryError> {
        let query = r#"
            SELECT * FROM third_party_sessions 
            WHERE session_id = $session_id AND client_secret = $client_secret
        "#;
        
        let mut result = self.db
            .query(query)
            .bind(("session_id", session_id.to_string()))
            .bind(("client_secret", client_secret.to_string()))
            .await?;

        let sessions: Vec<ThirdPartyValidationSession> = result.take(0)?;
        
        Ok(sessions.into_iter().next())
    }

    async fn mark_session_verified(&self, session_id: &str) -> Result<(), RepositoryError> {
        let validated_at = Utc::now().timestamp();
        
        let query = r#"
            UPDATE third_party_sessions SET 
                verified = true,
                validated_at = $validated_at
            WHERE session_id = $session_id
        "#;

        self.db
            .query(query)
            .bind(("session_id", session_id.to_string()))
            .bind(("validated_at", validated_at))
            .await?;

        info!("Marked 3PID session as verified: {}", session_id);
        Ok(())
    }

    async fn increment_session_attempts(&self, session_id: &str) -> Result<(), RepositoryError> {
        let query = r#"
            UPDATE third_party_sessions SET 
                attempt_count = attempt_count + 1
            WHERE session_id = $session_id
        "#;

        self.db
            .query(query)
            .bind(("session_id", session_id.to_string()))
            .await?;

        debug!("Incremented attempt count for session: {}", session_id);
        Ok(())
    }

    async fn associate_session_with_user(&self, session_id: &str, user_id: &str) -> Result<(), RepositoryError> {
        let query = r#"
            UPDATE third_party_sessions SET 
                user_id = $user_id
            WHERE session_id = $session_id
        "#;

        self.db
            .query(query)
            .bind(("session_id", session_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        info!("Associated session {} with user: {}", session_id, user_id);
        Ok(())
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), RepositoryError> {
        let query = "DELETE FROM third_party_sessions WHERE session_id = $session_id";

        self.db
            .query(query)
            .bind(("session_id", session_id.to_string()))
            .await?;

        debug!("Deleted 3PID validation session: {}", session_id);
        Ok(())
    }

    async fn get_sessions_by_address(&self, medium: &str, address: &str) -> Result<Vec<ThirdPartyValidationSession>, RepositoryError> {
        let query = r#"
            SELECT * FROM third_party_sessions 
            WHERE medium = $medium AND address = $address
            ORDER BY created_at DESC
        "#;
        
        let mut result = self.db
            .query(query)
            .bind(("medium", medium.to_string()))
            .bind(("address", address.to_string()))
            .await?;

        let sessions: Vec<ThirdPartyValidationSession> = result.take(0)?;
        
        Ok(sessions)
    }

    async fn cleanup_expired_sessions(&self) -> Result<u64, RepositoryError> {
        let now = Utc::now().timestamp();
        
        let query = "DELETE FROM third_party_sessions WHERE expires_at < $now";

        let mut result = self.db
            .query(query)
            .bind(("now", now))
            .await?;

        // Get count of deleted sessions
        let deleted_count = result.num_statements();
        
        info!("Cleaned up {} expired 3PID validation sessions", deleted_count);
        Ok(deleted_count as u64)
    }

    async fn get_active_sessions_for_user(&self, user_id: &str) -> Result<Vec<ThirdPartyValidationSession>, RepositoryError> {
        let now = Utc::now().timestamp();
        
        let query = r#"
            SELECT * FROM third_party_sessions 
            WHERE user_id = $user_id AND expires_at > $now
            ORDER BY created_at DESC
        "#;
        
        let mut result = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("now", now))
            .await?;

        let sessions: Vec<ThirdPartyValidationSession> = result.take(0)?;
        
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // Note: These tests would require a test database setup
    // They are provided as examples of how to test the repository

    #[tokio::test]
    #[ignore] // Ignore until test database is set up
    async fn test_create_and_get_session() {
        // This test would require setting up a test SurrealDB instance
        // let db = setup_test_db().await;
        // let repo = ThirdPartyValidationSessionRepository::new(db);
        
        // let session = ThirdPartyValidationSession::new(
        //     Uuid::new_v4().to_string(),
        //     "test_secret".to_string(),
        //     "email".to_string(),
        //     "test@example.com".to_string(),
        //     "token123".to_string(),
        //     Utc::now().timestamp() + 3600,
        // );
        
        // repo.create_session(&session).await.unwrap();
        // let retrieved = repo.get_session_by_id(&session.session_id).await.unwrap();
        
        // assert!(retrieved.is_some());
        // assert_eq!(retrieved.unwrap().session_id, session.session_id);
    }
}