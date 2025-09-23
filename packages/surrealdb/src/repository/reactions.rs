use crate::repository::error::RepositoryError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub event_id: String,
    pub room_id: String,
    pub user_id: String,
    pub reaction_key: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionAggregation {
    pub event_id: String,
    pub reaction_key: String,
    pub count: u64,
    pub users: Vec<String>,
}

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
        room_id: &str,
        event_id: &str,
        user_id: &str,
        reaction_key: &str,
    ) -> Result<String, RepositoryError> {
        // Validate reaction key
        if reaction_key.is_empty() || reaction_key.len() > 100 {
            return Err(RepositoryError::Validation {
                field: "reaction_key".to_string(),
                message: "Invalid reaction key length".to_string(),
            });
        }

        // Check for duplicate reaction
        if self.has_user_reaction(room_id, event_id, user_id, reaction_key).await? {
            return Err(RepositoryError::Conflict {
                message: "User has already reacted with this key".to_string(),
            });
        }

        let reaction_event_id = format!("${}", Uuid::new_v4());
        let timestamp = Utc::now().timestamp_millis();

        // Create reaction event
        let query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $user_id,
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

        self.db
            .query(query)
            .bind(("event_id", reaction_event_id.clone()))
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("target_event_id", event_id.to_string()))
            .bind(("reaction_key", reaction_key.to_string()))
            .bind(("timestamp", timestamp))
            .await?;

        // Store relation
        let relation_query = "
            CREATE event_relations SET
                event_id = $reaction_event_id,
                relates_to_event_id = $target_event_id,
                rel_type = 'm.annotation',
                room_id = $room_id,
                sender = $user_id,
                created_at = time::now()
        ";

        self.db
            .query(relation_query)
            .bind(("reaction_event_id", reaction_event_id.clone()))
            .bind(("target_event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        // Update aggregation
        self.update_reaction_aggregation(event_id, reaction_key, 1, user_id)
            .await?;

        Ok(reaction_event_id)
    }

    /// Remove a reaction from an event
    pub async fn remove_reaction(
        &self,
        room_id: &str,
        event_id: &str,
        user_id: &str,
        reaction_key: &str,
    ) -> Result<(), RepositoryError> {
        // Find the user's reaction event
        let reaction_event_query = "
            SELECT e.event_id FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $event_id
            AND r.rel_type = 'm.annotation'
            AND e.type = 'm.reaction'
            AND e.sender = $user_id
            AND e.content.m.relates_to.key = $reaction_key
            AND e.room_id = $room_id
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(reaction_event_query)
            .bind(("event_id", event_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("reaction_key", reaction_key.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let reactions: Vec<Value> = result.take(0)?;

        if let Some(reaction_data) = reactions.first() {
            if let Some(reaction_event_id) = reaction_data.get("event_id").and_then(|v| v.as_str())
            {
                // Create redaction event
                let redaction_event_id = format!("${}", Uuid::new_v4());
                let timestamp = Utc::now().timestamp_millis();

                let redaction_query = "
                    CREATE room_timeline_events SET
                        event_id = $redaction_event_id,
                        room_id = $room_id,
                        sender = $user_id,
                        type = 'm.room.redaction',
                        content = {
                            'reason': 'Reaction removed'
                        },
                        redacts = $reaction_event_id,
                        origin_server_ts = $timestamp,
                        created_at = time::now()
                ";

                self.db
                    .query(redaction_query)
                    .bind(("redaction_event_id", redaction_event_id))
                    .bind(("room_id", room_id.to_string()))
                    .bind(("user_id", user_id.to_string()))
                    .bind(("reaction_event_id", reaction_event_id.to_string()))
                    .bind(("timestamp", timestamp))
                    .await?;

                // Update aggregation
                self.update_reaction_aggregation(event_id, reaction_key, -1, user_id)
                    .await?;
            } else {
                return Err(RepositoryError::NotFound {
                    entity_type: "Reaction".to_string(),
                    id: format!("{}:{}:{}", event_id, user_id, reaction_key),
                });
            }
        } else {
            return Err(RepositoryError::NotFound {
                entity_type: "Reaction".to_string(),
                id: format!("{}:{}:{}", event_id, user_id, reaction_key),
            });
        }

        Ok(())
    }

    /// Get all reactions for an event
    pub async fn get_event_reactions(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Vec<Reaction>, RepositoryError> {
        let query = "
            SELECT 
                e.event_id,
                e.room_id,
                e.sender as user_id,
                e.content.m.relates_to.key as reaction_key,
                e.origin_server_ts as timestamp
            FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $event_id
            AND r.rel_type = 'm.annotation'
            AND e.type = 'm.reaction'
            AND e.room_id = $room_id
            ORDER BY e.origin_server_ts ASC
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let reactions_data: Vec<Value> = result.take(0)?;

        let mut reactions = Vec::new();
        for reaction_data in reactions_data {
            if let (
                Some(event_id),
                Some(room_id),
                Some(user_id),
                Some(reaction_key),
                Some(timestamp),
            ) = (
                reaction_data.get("event_id").and_then(|v| v.as_str()),
                reaction_data.get("room_id").and_then(|v| v.as_str()),
                reaction_data.get("user_id").and_then(|v| v.as_str()),
                reaction_data.get("reaction_key").and_then(|v| v.as_str()),
                reaction_data.get("timestamp").and_then(|v| v.as_i64()),
            ) {
                reactions.push(Reaction {
                    event_id: event_id.to_string(),
                    room_id: room_id.to_string(),
                    user_id: user_id.to_string(),
                    reaction_key: reaction_key.to_string(),
                    timestamp,
                });
            }
        }

        Ok(reactions)
    }

    /// Get reaction aggregation for an event
    pub async fn get_reaction_aggregation(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<ReactionAggregation, RepositoryError> {
        let query = "
            SELECT
                e.content.m.relates_to.key as reaction_key,
                COUNT(*) as count,
                array::group(e.sender) as users
            FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $event_id
            AND r.rel_type = 'm.annotation'
            AND e.type = 'm.reaction'
            AND e.room_id = $room_id
            GROUP BY e.content.m.relates_to.key
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let aggregations: Vec<Value> = result.take(0)?;

        // For now, return the first aggregation found
        // In a full implementation, you might want to return all aggregations
        if let Some(agg_data) = aggregations.first() {
            if let (Some(reaction_key), Some(count), Some(users)) = (
                agg_data.get("reaction_key").and_then(|v| v.as_str()),
                agg_data.get("count").and_then(|v| v.as_u64()),
                agg_data.get("users").and_then(|v| v.as_array()),
            ) {
                let user_list: Vec<String> =
                    users.iter().filter_map(|u| u.as_str().map(|s| s.to_string())).collect();

                return Ok(ReactionAggregation {
                    event_id: event_id.to_string(),
                    reaction_key: reaction_key.to_string(),
                    count,
                    users: user_list,
                });
            }
        }

        // Return empty aggregation if no reactions found
        Ok(ReactionAggregation {
            event_id: event_id.to_string(),
            reaction_key: String::new(),
            count: 0,
            users: Vec::new(),
        })
    }

    /// Get all reactions from a user in a room since a given time
    pub async fn get_user_reactions(
        &self,
        room_id: &str,
        user_id: &str,
        since: Option<&str>,
    ) -> Result<Vec<Reaction>, RepositoryError> {
        let query = if since.is_some() {
            "
            SELECT 
                e.event_id,
                e.room_id,
                e.sender as user_id,
                e.content.m.relates_to.key as reaction_key,
                e.origin_server_ts as timestamp
            FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE e.type = 'm.reaction'
            AND e.room_id = $room_id
            AND e.sender = $user_id
            AND e.origin_server_ts > $since
            ORDER BY e.origin_server_ts ASC
            "
        } else {
            "
            SELECT 
                e.event_id,
                e.room_id,
                e.sender as user_id,
                e.content.m.relates_to.key as reaction_key,
                e.origin_server_ts as timestamp
            FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE e.type = 'm.reaction'
            AND e.room_id = $room_id
            AND e.sender = $user_id
            ORDER BY e.origin_server_ts ASC
            "
        };

        let mut query_builder = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()));

        if let Some(since_token) = since {
            // Parse since token as timestamp
            let since_ts = since_token.parse::<i64>().unwrap_or(0);
            query_builder = query_builder.bind(("since", since_ts));
        }

        let mut result = query_builder.await?;
        let reactions_data: Vec<Value> = result.take(0)?;

        let mut reactions = Vec::new();
        for reaction_data in reactions_data {
            if let (
                Some(event_id),
                Some(room_id),
                Some(user_id),
                Some(reaction_key),
                Some(timestamp),
            ) = (
                reaction_data.get("event_id").and_then(|v| v.as_str()),
                reaction_data.get("room_id").and_then(|v| v.as_str()),
                reaction_data.get("user_id").and_then(|v| v.as_str()),
                reaction_data.get("reaction_key").and_then(|v| v.as_str()),
                reaction_data.get("timestamp").and_then(|v| v.as_i64()),
            ) {
                reactions.push(Reaction {
                    event_id: event_id.to_string(),
                    room_id: room_id.to_string(),
                    user_id: user_id.to_string(),
                    reaction_key: reaction_key.to_string(),
                    timestamp,
                });
            }
        }

        Ok(reactions)
    }

    /// Validate if user has permission to react to an event
    pub async fn validate_reaction_permissions(
        &self,
        room_id: &str,
        user_id: &str,
        event_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if user is a member of the room
        let membership_query = "
            SELECT membership FROM membership
            WHERE room_id = $room_id AND user_id = $user_id
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(membership_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let memberships: Vec<Value> = result.take(0)?;

        if let Some(membership_data) = memberships.first() {
            if let Some(membership) = membership_data.get("membership").and_then(|v| v.as_str()) {
                // User can react if they are joined
                return Ok(membership == "join");
            }
        }

        // Also check if the target event exists
        let event_query = "
            SELECT event_id FROM room_timeline_events
            WHERE event_id = $event_id AND room_id = $room_id
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(event_query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let events: Vec<Value> = result.take(0)?;

        Ok(!events.is_empty())
    }

    /// Clean up reactions for a redacted event
    pub async fn cleanup_reactions_for_redacted_event(
        &self,
        _room_id: &str,
        event_id: &str,
    ) -> Result<(), RepositoryError> {
        // Remove all reaction aggregations for the redacted event
        let cleanup_aggregation_query = "
            DELETE FROM reaction_aggregations
            WHERE target_event_id = $event_id
        ";

        self.db
            .query(cleanup_aggregation_query)
            .bind(("event_id", event_id.to_string()))
            .await?;

        // Remove event relations for reactions to this event
        let cleanup_relations_query = "
            DELETE FROM event_relations
            WHERE relates_to_event_id = $event_id AND rel_type = 'm.annotation'
        ";

        self.db
            .query(cleanup_relations_query)
            .bind(("event_id", event_id.to_string()))
            .await?;

        Ok(())
    }

    /// Check if user already has a specific reaction
    async fn has_user_reaction(
        &self,
        room_id: &str,
        event_id: &str,
        user_id: &str,
        reaction_key: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT COUNT(*) as count FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $event_id
            AND r.rel_type = 'm.annotation'
            AND e.type = 'm.reaction'
            AND e.sender = $user_id
            AND e.content.m.relates_to.key = $reaction_key
            AND e.room_id = $room_id
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("reaction_key", reaction_key.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let counts: Vec<Value> = result.take(0)?;

        if let Some(count_data) = counts.first() {
            if let Some(count) = count_data.get("count").and_then(|v| v.as_u64()) {
                return Ok(count > 0);
            }
        }

        Ok(false)
    }

    /// Update reaction aggregation data
    async fn update_reaction_aggregation(
        &self,
        event_id: &str,
        reaction_key: &str,
        delta: i64,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE reaction_aggregations SET
                count = math::max(0, count + $delta),
                users = IF($delta > 0,
                    array::union(users, [$user_id]),
                    array::difference(users, [$user_id])
                ),
                updated_at = time::now()
            WHERE target_event_id = $event_id AND reaction_key = $reaction_key
            ELSE CREATE reaction_aggregations SET
                id = rand::uuid(),
                target_event_id = $event_id,
                reaction_key = $reaction_key,
                count = math::max(0, $delta),
                users = IF($delta > 0, [$user_id], []),
                created_at = time::now(),
                updated_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .bind(("reaction_key", reaction_key.to_string()))
            .bind(("delta", delta))
            .bind(("user_id", user_id.to_string()))
            .await?;

        Ok(())
    }
}
