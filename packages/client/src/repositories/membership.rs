use anyhow::Result;
use futures_util::{Stream, StreamExt};
use matryx_entity::Membership;
use matryx_surrealdb::repository::RepositoryError;
use std::pin::Pin;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),
}

/// Repository for handling room membership operations using server-side repository
#[derive(Clone)]
pub struct MembershipRepository {
    membership_repo: matryx_surrealdb::repository::MembershipRepository,
    user_id: String,
}

impl MembershipRepository {
    pub fn new(db: Surreal<Any>, user_id: String) -> Self {
        Self {
            membership_repo: matryx_surrealdb::repository::MembershipRepository::new(db),
            user_id,
        }
    }

    /// Get all memberships for a user using server repository
    pub async fn get_user_memberships(
        &self,
        user_id: &str,
    ) -> Result<Vec<Membership>, ClientError> {
        let memberships = self.membership_repo.get_user_rooms(user_id).await?;
        Ok(memberships)
    }

    /// Subscribe to membership changes for a user using server repository
    pub async fn subscribe_membership_changes<'a>(
        &'a self,
        user_id: &'a str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Vec<Membership>, ClientError>> + Send + 'a>>,
        ClientError,
    > {
        // Get initial memberships for the user (available for immediate access)
        let _initial_memberships = self.get_user_memberships(user_id).await?;
        
        let stream = self.membership_repo.subscribe_user_membership(user_id).await?.map(
            move |membership_result| -> Result<Vec<Membership>, ClientError> {
                let membership = membership_result.map_err(ClientError::Repository)?;
                // Return the new membership in a vec for stream consistency
                // Subscribers can fetch all memberships via get_user_memberships if needed
                Ok(vec![membership])
            },
        );

        Ok(Box::pin(stream))
    }

    /// Get memberships for the current user
    pub async fn get_current_user_memberships(&self) -> Result<Vec<Membership>, ClientError> {
        self.get_user_memberships(&self.user_id).await
    }

    /// Subscribe to membership changes for the current user
    pub async fn subscribe_current_user_membership_changes<'a>(
        &'a self,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Vec<Membership>, ClientError>> + Send + 'a>>,
        ClientError,
    > {
        // Get initial memberships for the current user
        let _initial_memberships = self.get_current_user_memberships().await?;
        
        let stream = self.membership_repo.subscribe_user_membership(&self.user_id).await?.map(
            |membership_result| -> Result<Vec<Membership>, ClientError> {
                let membership = membership_result.map_err(ClientError::Repository)?;
                // Return the new membership in a vec for stream consistency
                // Subscribers can fetch all memberships via get_current_user_memberships if needed
                Ok(vec![membership])
            },
        );

        Ok(Box::pin(stream))
    }
}
