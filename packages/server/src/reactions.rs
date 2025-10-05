//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;
use matryx_surrealdb::repository::{
    error::RepositoryError,
    event::EventRepository,
    reactions::{ReactionAggregation, ReactionSummary, ReactionsRepository},
};

#[derive(Debug, thiserror::Error)]
pub enum ReactionError {
    #[error("Invalid reaction key: {0}")]
    InvalidReactionKey(String),
    #[error("User has already reacted with this key")]
    DuplicateReaction,
    #[error("Target event not found")]
    TargetEventNotFound,
    #[error("Reaction not found")]
    ReactionNotFound,
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
}

/// Statistics about reactions in a room for monitoring and analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionStats {
    pub total_reactions: u64,
    pub unique_reactions: u64,
    pub top_reactions: HashMap<String, u64>,
    pub active_users: u64,
}

pub struct ReactionManager {
    reactions_repo: ReactionsRepository<surrealdb::engine::any::Any>,
    event_repo: EventRepository,
}

impl ReactionManager {
    pub fn new(state: &AppState) -> Self {
        Self {
            reactions_repo: ReactionsRepository::new(state.db.clone()),
            event_repo: EventRepository::new(state.db.clone()),
        }
    }

    /// Validate that a reaction is allowed
    pub async fn validate_reaction(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        sender: &str,
    ) -> Result<(), ReactionError> {
        info!("Validating reaction {} for event {} by {}", reaction_key, target_event_id, sender);

        // Validate reaction key format (basic Unicode emoji or shortcode)
        if reaction_key.is_empty() || reaction_key.len() > 100 {
            return Err(ReactionError::InvalidReactionKey(
                "Reaction key must be 1-100 characters".to_string(),
            ));
        }

        // Check if target event exists
        match self.event_repo.get_by_id(target_event_id).await? {
            Some(_) => {},
            None => {
                warn!("Target event {} not found for reaction", target_event_id);
                return Err(ReactionError::TargetEventNotFound);
            },
        }

        // Check for duplicate reaction
        if self
            .reactions_repo
            .has_user_reacted(sender, target_event_id, reaction_key)
            .await?
        {
            warn!(
                "User {} already reacted with {} to event {}",
                sender, reaction_key, target_event_id
            );
            return Err(ReactionError::DuplicateReaction);
        }

        info!("Reaction validation passed");
        Ok(())
    }

    /// Add a reaction to an event
    pub async fn add_reaction(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        sender: &str,
        room_id: &str,
    ) -> Result<String, ReactionError> {
        let reaction_event_id = format!("$reaction_{}_{}", Uuid::new_v4(), target_event_id);

        self.reactions_repo
            .add_reaction(&reaction_event_id, room_id, sender, target_event_id, reaction_key)
            .await?;

        info!("Successfully added reaction {} to event {}", reaction_key, target_event_id);
        Ok(reaction_event_id)
    }

    /// Remove a reaction from an event
    pub async fn remove_reaction(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        sender: &str,
    ) -> Result<(), ReactionError> {
        self.reactions_repo
            .remove_reaction(sender, target_event_id, reaction_key)
            .await?;

        info!("Successfully removed reaction {} from event {}", reaction_key, target_event_id);
        Ok(())
    }

    /// Get reaction summary for an event
    pub async fn get_reaction_summary(
        &self,
        target_event_id: &str,
    ) -> Result<ReactionSummary, ReactionError> {
        let summary = self.reactions_repo.get_reaction_summary(target_event_id).await?;

        Ok(summary)
    }

    /// Get all reactions by a specific user
    pub async fn get_user_reactions(
        &self,
        user_id: &str,
        room_id: Option<&str>,
    ) -> Result<Vec<Value>, ReactionError> {
        let reactions = self.reactions_repo.get_user_reactions(user_id, room_id).await?;

        Ok(reactions)
    }

    /// Get reaction statistics for a room using HashMap for aggregation
    pub async fn get_reaction_stats(&self, room_id: &str) -> Result<ReactionStats, ReactionError> {
        info!("Calculating reaction statistics for room {}", room_id);

        // Get all reactions for the room and aggregate them
        let reactions = self.reactions_repo.get_room_reactions(room_id).await?;

        let mut reaction_counts: HashMap<String, u64> = HashMap::new();
        let mut users_set: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut total_reactions = 0u64;

        // Process reactions and build statistics
        for reaction in reactions {
            if let (Some(key), Some(sender)) = (
                reaction.get("reaction_key").and_then(|v| v.as_str()),
                reaction.get("sender").and_then(|v| v.as_str()),
            ) {
                *reaction_counts.entry(key.to_string()).or_insert(0) += 1;
                users_set.insert(sender.to_string());
                total_reactions += 1;
            }
        }

        let unique_reactions = reaction_counts.len() as u64;
        let active_users = users_set.len() as u64;

        // Keep only top 10 reactions for the response
        let mut sorted_reactions: Vec<_> = reaction_counts.into_iter().collect();
        sorted_reactions.sort_by(|a, b| b.1.cmp(&a.1));
        let top_reactions: HashMap<String, u64> = sorted_reactions.into_iter().take(10).collect();

        let stats = ReactionStats {
            total_reactions,
            unique_reactions,
            top_reactions,
            active_users,
        };

        info!(
            "Room {} has {} total reactions, {} unique reactions, {} active users",
            room_id, stats.total_reactions, stats.unique_reactions, stats.active_users
        );

        Ok(stats)
    }

    /// Get detailed reaction aggregation data using ReactionAggregation
    pub async fn get_reaction_aggregation(
        &self,
        target_event_id: &str,
    ) -> Result<Vec<ReactionAggregation>, ReactionError> {
        info!("Getting reaction aggregation for event {}", target_event_id);

        let aggregations = self.reactions_repo.get_reaction_aggregations(target_event_id).await?;

        Ok(aggregations)
    }
}

// Note: Default implementation removed for production safety.
// ReactionManager must be constructed with proper AppState using new().

// Re-export types for convenience - removed duplicate imports
