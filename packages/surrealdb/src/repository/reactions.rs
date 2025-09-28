use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};
use tracing::info;

use crate::repository::error::RepositoryError;

#[derive(Serialize, Deserialize)]
pub struct ReactionAggregation {
    pub key: String,
    pub count: u64,
    pub users: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ReactionSummary {
    pub target_event_id: String,
    pub reactions: HashMap<String, ReactionAggregation>,
}

/// Repository for managing Matrix reactions (m.reaction events)
pub struct ReactionsRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ReactionsRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Add a reaction to an event
    pub async fn add_reaction(
        &self,
        reaction_event_id: &str,
        room_id: &str,
        sender: &str,
        target_event_id: &str,
        reaction_key: &str,
    ) -> Result<(), RepositoryError> {
        info!("Adding reaction {} to event {} by {}", reaction_key, target_event_id, sender);

        let query = "
            CREATE event_relations SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                relates_to_event_id = $target_event_id,
                rel_type = 'm.annotation',
                reaction_key = $reaction_key,
                created_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("event_id", reaction_event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .bind(("target_event_id", target_event_id.to_string()))
            .bind(("reaction_key", reaction_key.to_string()))
            .await?;

        Ok(())
    }

    /// Remove a reaction from an event
    pub async fn remove_reaction(
        &self,
        sender: &str,
        target_event_id: &str,
        reaction_key: &str,
    ) -> Result<(), RepositoryError> {
        info!("Removing reaction {} from event {} by {}", reaction_key, target_event_id, sender);

        let query = "
            DELETE event_relations 
            WHERE sender = $sender 
            AND relates_to_event_id = $target_event_id 
            AND reaction_key = $reaction_key
            AND rel_type = 'm.annotation'
        ";

        self.db
            .query(query)
            .bind(("sender", sender.to_string()))
            .bind(("target_event_id", target_event_id.to_string()))
            .bind(("reaction_key", reaction_key.to_string()))
            .await?;

        Ok(())
    }

    /// Get reaction summary for an event
    pub async fn get_reaction_summary(
        &self,
        target_event_id: &str,
    ) -> Result<ReactionSummary, RepositoryError> {
        let query = "
            SELECT 
                reaction_key,
                count() as count,
                array::group(sender) as users
            FROM event_relations 
            WHERE relates_to_event_id = $target_event_id 
            AND rel_type = 'm.annotation'
            AND reaction_key IS NOT NONE
            GROUP BY reaction_key
        ";

        let mut result = self.db
            .query(query)
            .bind(("target_event_id", target_event_id.to_string()))
            .await?;

        let reactions_data: Vec<Value> = result.take(0)?;
        let mut reactions = HashMap::new();

        for reaction_data in reactions_data {
            if let (Some(key), Some(count), Some(users)) = (
                reaction_data.get("reaction_key").and_then(|v| v.as_str()),
                reaction_data.get("count").and_then(|v| v.as_u64()),
                reaction_data.get("users").and_then(|v| v.as_array()),
            ) {
                let user_list: Vec<String> = users
                    .iter()
                    .filter_map(|u| u.as_str().map(|s| s.to_string()))
                    .collect();

                reactions.insert(
                    key.to_string(),
                    ReactionAggregation {
                        key: key.to_string(),
                        count,
                        users: user_list,
                    },
                );
            }
        }

        Ok(ReactionSummary {
            target_event_id: target_event_id.to_string(),
            reactions,
        })
    }

    /// Get all reactions by a specific user
    pub async fn get_user_reactions(
        &self,
        user_id: &str,
        room_id: Option<&str>,
    ) -> Result<Vec<Value>, RepositoryError> {
        let query = if let Some(_room_id) = room_id {
            "
                SELECT * FROM event_relations 
                WHERE sender = $user_id 
                AND room_id = $room_id
                AND rel_type = 'm.annotation'
                AND reaction_key IS NOT NONE
                ORDER BY created_at DESC
            "
        } else {
            "
                SELECT * FROM event_relations 
                WHERE sender = $user_id 
                AND rel_type = 'm.annotation'
                AND reaction_key IS NOT NONE
                ORDER BY created_at DESC
            "
        };

        let mut query_builder = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()));

        if let Some(room_id) = room_id {
            query_builder = query_builder.bind(("room_id", room_id.to_string()));
        }

        let mut result = query_builder.await?;
        let reactions: Vec<Value> = result.take(0)?;
        Ok(reactions)
    }

    /// Check if user has already reacted with a specific key
    pub async fn has_user_reacted(
        &self,
        user_id: &str,
        target_event_id: &str,
        reaction_key: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() as count FROM event_relations 
            WHERE sender = $user_id 
            AND relates_to_event_id = $target_event_id 
            AND reaction_key = $reaction_key
            AND rel_type = 'm.annotation'
        ";

        let mut result = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("target_event_id", target_event_id.to_string()))
            .bind(("reaction_key", reaction_key.to_string()))
            .await?;

        let count_result: Vec<Value> = result.take(0)?;
        let count = count_result
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    /// Get all reactions for a specific room
    pub async fn get_room_reactions(
        &self,
        room_id: &str,
    ) -> Result<Vec<Value>, RepositoryError> {
        let query = "
            SELECT * FROM event_relations 
            WHERE room_id = $room_id
            AND rel_type = 'm.annotation'
            AND reaction_key IS NOT NONE
            ORDER BY created_at DESC
        ";

        let mut result = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let reactions: Vec<Value> = result.take(0)?;
        Ok(reactions)
    }

    /// Get reaction aggregations for a specific event
    pub async fn get_reaction_aggregations(
        &self,
        target_event_id: &str,
    ) -> Result<Vec<ReactionAggregation>, RepositoryError> {
        let query = "
            SELECT 
                reaction_key,
                count() as count,
                array::group(sender) as users
            FROM event_relations 
            WHERE relates_to_event_id = $target_event_id 
            AND rel_type = 'm.annotation'
            AND reaction_key IS NOT NONE
            GROUP BY reaction_key
        ";

        let mut result = self.db
            .query(query)
            .bind(("target_event_id", target_event_id.to_string()))
            .await?;

        let reactions_data: Vec<Value> = result.take(0)?;
        let mut aggregations = Vec::new();

        for reaction_data in reactions_data {
            if let (Some(key), Some(count), Some(users)) = (
                reaction_data.get("reaction_key").and_then(|v| v.as_str()),
                reaction_data.get("count").and_then(|v| v.as_u64()),
                reaction_data.get("users").and_then(|v| v.as_array()),
            ) {
                let user_list: Vec<String> = users
                    .iter()
                    .filter_map(|u| u.as_str().map(|s| s.to_string()))
                    .collect();

                aggregations.push(ReactionAggregation {
                    key: key.to_string(),
                    count,
                    users: user_list,
                });
            }
        }

        Ok(aggregations)
    }
}