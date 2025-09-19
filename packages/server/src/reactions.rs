use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;

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
}

#[derive(Serialize, Deserialize)]
pub struct Event {
    pub event_id: String,
    pub room_id: String,
    pub sender: String,
    pub content: Value,
    pub origin_server_ts: u64,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct ReactionAggregation {
    pub key: String,
    pub count: u64,
    pub users: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ReactionSummary {
    pub reactions: HashMap<String, ReactionAggregation>,
    pub total_reactions: u64,
}

pub struct ReactionManager;

impl ReactionManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn validate_reaction(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        sender: &str,
        state: &AppState,
    ) -> Result<(), ReactionError> {
        info!("Validating reaction '{}' from {} for event {}", reaction_key, sender, target_event_id);

        // Validate reaction key (emoji or custom)
        self.validate_reaction_key(reaction_key)?;
        
        // Check target event exists
        self.get_event(target_event_id, state).await?;
        
        // Check for duplicate reaction from same user
        let existing = self.get_user_reaction(target_event_id, reaction_key, sender, state).await?;
        if existing.is_some() {
            warn!("User {} already has reaction '{}' on event {}", sender, reaction_key, target_event_id);
            return Err(ReactionError::DuplicateReaction);
        }
        
        info!("Reaction validation successful");
        Ok(())
    }
    
    pub async fn add_reaction(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        sender: &str,
        room_id: &str,
        state: &AppState,
    ) -> Result<String, ReactionError> {
        info!("Adding reaction '{}' from {} for event {}", reaction_key, sender, target_event_id);

        // Create reaction event
        let reaction_event_id = format!("${}", Uuid::new_v4());
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        let query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                type = 'm.reaction',
                content = {
                    'm.relates_to': {
                        'rel_type': 'm.annotation',
                        'event_id': $target_event_id,
                        'key': $reaction_key
                    }
                },
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("event_id", &reaction_event_id))
            .bind(("room_id", room_id))
            .bind(("sender", sender))
            .bind(("target_event_id", target_event_id))
            .bind(("reaction_key", reaction_key))
            .bind(("timestamp", timestamp))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        // Store relation
        let relation_query = "
            CREATE event_relations SET
                event_id = $reaction_event_id,
                relates_to_event_id = $target_event_id,
                rel_type = 'm.annotation',
                room_id = $room_id,
                sender = $sender,
                created_at = time::now()
        ";
        
        state.db
            .query(relation_query)
            .bind(("reaction_event_id", &reaction_event_id))
            .bind(("target_event_id", target_event_id))
            .bind(("room_id", room_id))
            .bind(("sender", sender))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;
        
        // Update aggregation
        self.update_reaction_aggregation(target_event_id, reaction_key, 1, sender, state).await?;
        
        info!("Successfully added reaction {} for event {}", reaction_event_id, target_event_id);
        Ok(reaction_event_id)
    }

    pub async fn remove_reaction(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        user_id: &str,
        state: &AppState,
    ) -> Result<(), ReactionError> {
        info!("Removing reaction '{}' from {} for event {}", reaction_key, user_id, target_event_id);

        // Find the user's reaction event
        let reaction_event = self.get_user_reaction(target_event_id, reaction_key, user_id, state).await?;
        
        if let Some(event) = reaction_event {
            // Redact the reaction event
            self.redact_event(&event.event_id, user_id, state).await?;
            
            // Update aggregation
            self.update_reaction_aggregation(target_event_id, reaction_key, -1, user_id, state).await?;
            
            info!("Successfully removed reaction for event {}", target_event_id);
        } else {
            return Err(ReactionError::ReactionNotFound);
        }
        
        Ok(())
    }

    pub async fn get_reaction_summary(
        &self,
        target_event_id: &str,
        state: &AppState,
    ) -> Result<ReactionSummary, ReactionError> {
        let query = "
            SELECT 
                content.m.relates_to.key as reaction_key,
                COUNT(*) as count,
                array::group(sender) as users
            FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $target_event_id 
            AND r.rel_type = 'm.annotation'
            AND e.type = 'm.reaction'
            GROUP BY content.m.relates_to.key
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("target_event_id", target_event_id))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        let aggregations: Vec<Value> = result.take(0)
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        let mut reactions = HashMap::new();
        let mut total_reactions = 0u64;

        for agg_data in aggregations {
            if let (Some(key), Some(count), Some(users)) = (
                agg_data.get("reaction_key").and_then(|k| k.as_str()),
                agg_data.get("count").and_then(|c| c.as_u64()),
                agg_data.get("users").and_then(|u| u.as_array())
            ) {
                let user_list: Vec<String> = users
                    .iter()
                    .filter_map(|u| u.as_str().map(|s| s.to_string()))
                    .collect();

                reactions.insert(key.to_string(), ReactionAggregation {
                    key: key.to_string(),
                    count,
                    users: user_list,
                });

                total_reactions += count;
            }
        }

        Ok(ReactionSummary {
            reactions,
            total_reactions,
        })
    }

    pub async fn get_user_reactions(
        &self,
        target_event_id: &str,
        user_id: &str,
        state: &AppState,
    ) -> Result<Vec<String>, ReactionError> {
        let query = "
            SELECT content.m.relates_to.key as reaction_key
            FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $target_event_id 
            AND r.rel_type = 'm.annotation'
            AND e.type = 'm.reaction'
            AND e.sender = $user_id
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("target_event_id", target_event_id))
            .bind(("user_id", user_id))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        let reactions: Vec<Value> = result.take(0)
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        let reaction_keys: Vec<String> = reactions
            .into_iter()
            .filter_map(|r| r.get("reaction_key").and_then(|k| k.as_str()).map(|s| s.to_string()))
            .collect();

        Ok(reaction_keys)
    }

    fn validate_reaction_key(&self, key: &str) -> Result<(), ReactionError> {
        // Basic validation - key should not be empty and should be reasonable length
        if key.is_empty() {
            return Err(ReactionError::InvalidReactionKey("Reaction key cannot be empty".to_string()));
        }

        if key.len() > 100 {
            return Err(ReactionError::InvalidReactionKey("Reaction key too long".to_string()));
        }

        // Allow emoji and custom reaction keys
        // In a full implementation, you might want to validate Unicode emoji sequences
        Ok(())
    }

    async fn get_user_reaction(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        user_id: &str,
        state: &AppState,
    ) -> Result<Option<Event>, ReactionError> {
        let query = "
            SELECT e.* FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $target_event_id 
            AND r.rel_type = 'm.annotation'
            AND e.type = 'm.reaction'
            AND e.sender = $user_id
            AND content.m.relates_to.key = $reaction_key
            LIMIT 1
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("target_event_id", target_event_id))
            .bind(("user_id", user_id))
            .bind(("reaction_key", reaction_key))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        let events: Vec<Value> = result.take(0)
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        if let Some(event_data) = events.first() {
            let event = self.value_to_event(event_data.clone())?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    async fn get_event(
        &self,
        event_id: &str,
        state: &AppState,
    ) -> Result<Event, ReactionError> {
        let query = "
            SELECT event_id, room_id, sender, content, origin_server_ts, type
            FROM room_timeline_events 
            WHERE event_id = $event_id
            LIMIT 1
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("event_id", event_id))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        let events: Vec<Value> = result.take(0)
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;
        
        if let Some(event_data) = events.first() {
            self.value_to_event(event_data.clone())
        } else {
            Err(ReactionError::TargetEventNotFound)
        }
    }

    fn value_to_event(&self, event_data: Value) -> Result<Event, ReactionError> {
        Ok(Event {
            event_id: event_data["event_id"].as_str().unwrap_or("").to_string(),
            room_id: event_data["room_id"].as_str().unwrap_or("").to_string(),
            sender: event_data["sender"].as_str().unwrap_or("").to_string(),
            content: event_data["content"].clone(),
            origin_server_ts: event_data["origin_server_ts"].as_u64().unwrap_or(0),
            event_type: event_data["type"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn update_reaction_aggregation(
        &self,
        target_event_id: &str,
        reaction_key: &str,
        delta: i64,
        user_id: &str,
        state: &AppState,
    ) -> Result<(), ReactionError> {
        // Update or create aggregation record
        let query = "
            UPDATE reaction_aggregations SET 
                count = math::max(0, count + $delta),
                users = IF($delta > 0, 
                    array::union(users, [$user_id]), 
                    array::difference(users, [$user_id])
                ),
                updated_at = time::now()
            WHERE target_event_id = $target_event_id AND reaction_key = $reaction_key
            ELSE CREATE reaction_aggregations SET
                id = rand::uuid(),
                target_event_id = $target_event_id,
                reaction_key = $reaction_key,
                count = math::max(0, $delta),
                users = IF($delta > 0, [$user_id], []),
                created_at = time::now(),
                updated_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("target_event_id", target_event_id))
            .bind(("reaction_key", reaction_key))
            .bind(("delta", delta))
            .bind(("user_id", user_id))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn redact_event(
        &self,
        event_id: &str,
        redactor_id: &str,
        state: &AppState,
    ) -> Result<(), ReactionError> {
        let redaction_event_id = format!("${}", Uuid::new_v4());
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        let query = "
            CREATE room_timeline_events SET
                event_id = $redaction_event_id,
                room_id = (SELECT room_id FROM room_timeline_events WHERE event_id = $event_id LIMIT 1),
                sender = $redactor_id,
                type = 'm.room.redaction',
                content = {
                    'reason': 'Reaction removed'
                },
                redacts = $event_id,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("redaction_event_id", redaction_event_id))
            .bind(("event_id", event_id))
            .bind(("redactor_id", redactor_id))
            .bind(("timestamp", timestamp))
            .await
            .map_err(|e| ReactionError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}

impl Default for ReactionManager {
    fn default() -> Self {
        Self::new()
    }
}