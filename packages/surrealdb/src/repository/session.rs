use crate::repository::error::RepositoryError;
use matryx_entity::types::Session;
use surrealdb::{Surreal, engine::any::Any};

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
}
