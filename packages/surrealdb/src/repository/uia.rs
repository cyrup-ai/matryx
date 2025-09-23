use crate::repository::error::RepositoryError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

/// UIA session stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiaSession {
    pub session_id: String,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
    pub flows: Vec<UiaFlow>,
    pub completed_stages: Vec<String>,
    pub current_stage: Option<String>,
    pub auth_data: HashMap<String, serde_json::Value>,
    pub params: HashMap<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub completed: bool,
}

/// UIA flow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiaFlow {
    pub stages: Vec<String>,
}

/// UIA authentication stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStage {
    pub stage_type: String,
    pub params: HashMap<String, serde_json::Value>,
}

/// UIA session statistics for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct UiaSessionStats {
    pub total_sessions: u64,
    pub completed_sessions: u64,
    pub expired_sessions: u64,
    pub active_sessions: u64,
}

pub struct UiaRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> UiaRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub fn get_db(&self) -> &Surreal<C> {
        &self.db
    }

    /// Create a new UIA session
    pub async fn create_session(
        &self,
        user_id: Option<&str>,
        device_id: Option<&str>,
        flows: Vec<UiaFlow>,
        params: HashMap<String, serde_json::Value>,
        session_lifetime: Duration,
    ) -> Result<UiaSession, RepositoryError> {
        let session_id = format!("uia_{}", Uuid::new_v4());
        let now = Utc::now();

        let session = UiaSession {
            session_id: session_id.clone(),
            user_id: user_id.map(|s| s.to_string()),
            device_id: device_id.map(|s| s.to_string()),
            flows,
            completed_stages: Vec::new(),
            current_stage: None,
            auth_data: HashMap::new(),
            params,
            created_at: now,
            expires_at: now + session_lifetime,
            completed: false,
        };

        let _: Option<UiaSession> = self
            .db
            .create(("uia_sessions", &session_id))
            .content(session.clone())
            .await?;

        Ok(session)
    }

    /// Get UIA session by ID
    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> Result<Option<UiaSession>, RepositoryError> {
        let session: Option<UiaSession> = self.db.select(("uia_sessions", session_id)).await?;
        Ok(session)
    }

    /// Update UIA session
    pub async fn update_session(&self, session: &UiaSession) -> Result<(), RepositoryError> {
        let _: Option<UiaSession> = self
            .db
            .update(("uia_sessions", &session.session_id))
            .content(session.clone())
            .await?;
        Ok(())
    }

    /// Delete UIA session
    pub async fn delete_session(&self, session_id: &str) -> Result<(), RepositoryError> {
        let _: Option<UiaSession> = self.db.delete(("uia_sessions", session_id)).await?;
        Ok(())
    }

    /// Check if UIA session is valid (exists and not expired)
    pub async fn is_session_valid(&self, session_id: &str) -> Result<bool, RepositoryError> {
        if let Some(session) = self.get_session(session_id).await? {
            Ok(Utc::now() <= session.expires_at)
        } else {
            Ok(false)
        }
    }

    /// Get all active UIA sessions for a user
    pub async fn get_user_sessions(
        &self,
        user_id: &str,
    ) -> Result<Vec<UiaSession>, RepositoryError> {
        let query = "
            SELECT * FROM uia_sessions 
            WHERE user_id = $user_id AND expires_at > datetime::now()
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let sessions: Vec<UiaSession> = result.take(0)?;
        Ok(sessions)
    }

    /// Complete a stage in a UIA session
    pub async fn complete_stage(
        &self,
        session_id: &str,
        stage: &str,
    ) -> Result<UiaSession, RepositoryError> {
        let session: Option<UiaSession> = self.db.select(("uia_sessions", session_id)).await?;

        if let Some(mut session) = session {
            if !session.completed_stages.contains(&stage.to_string()) {
                session.completed_stages.push(stage.to_string());

                // Check if all required stages are completed
                let all_completed = session.flows.iter().any(|flow| {
                    flow.stages
                        .iter()
                        .all(|required_stage| session.completed_stages.contains(required_stage))
                });

                if all_completed {
                    session.completed = true;
                }

                let updated: Option<UiaSession> = self
                    .db
                    .update(("uia_sessions", session_id))
                    .content(session.clone())
                    .await?;
                updated.ok_or_else(|| {
                    RepositoryError::NotFound {
                        entity_type: "UiaSession".to_string(),
                        id: session_id.to_string(),
                    }
                })
            } else {
                Ok(session)
            }
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "UiaSession".to_string(),
                id: session_id.to_string(),
            })
        }
    }

    /// Invalidate all sessions for a user
    pub async fn invalidate_user_sessions(&self, user_id: &str) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM uia_sessions WHERE user_id = $user_id";
        let mut response = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let deleted_count: Option<u64> = response.take(0).unwrap_or(Some(0));
        Ok(deleted_count.unwrap_or(0))
    }

    /// Clean up expired UIA sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM uia_sessions WHERE expires_at < datetime::now()";
        let mut response = self.db.query(query).await?;
        let deleted_count: Option<u64> = response.take(0).unwrap_or(Some(0));
        Ok(deleted_count.unwrap_or(0))
    }

    /// Get UIA session statistics
    pub async fn get_session_stats(&self) -> Result<UiaSessionStats, RepositoryError> {
        let query = "
            SELECT 
                count() as total_sessions,
                count(completed = true) as completed_sessions,
                count(expires_at < datetime::now()) as expired_sessions,
                count(expires_at >= datetime::now() AND completed = false) as active_sessions
            FROM uia_sessions
        ";

        let mut response = self.db.query(query).await?;
        let stats: Option<UiaSessionStats> = response.take(0)?;
        stats.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "UiaSessionStats".to_string(),
                id: "default".to_string(),
            }
        })
    }

    /// Revoke all UIA sessions for a user
    pub async fn revoke_user_sessions(&self, user_id: &str) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM uia_sessions WHERE user_id = $user_id";
        let mut response = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let deleted_count: Option<u64> = response.take(0).unwrap_or(Some(0));
        Ok(deleted_count.unwrap_or(0))
    }
}
