use crate::repository::error::RepositoryError;
use chrono::Utc;
use matryx_entity::types::Event;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

/// Direction for relation queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationDirection {
    /// Events that relate to the given event (children/replies)
    Forward,
    /// Events that the given event relates to (parents/references)
    Backward,
    /// Both directions
    Both,
}

/// Response for event relations query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationsResponse {
    pub chunk: Vec<Event>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
}

/// Aggregated relation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationAggregation {
    pub reactions: HashMap<String, u32>, // emoji -> count
    pub replies: u32,
    pub edits: u32,
    pub annotations: u32,
    pub threads: u32,
}

/// Event relation record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRelation {
    pub id: String,
    pub room_id: String,
    pub parent_event_id: String,
    pub child_event_id: String,
    pub rel_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Repository for managing event relations (replies, reactions, edits, etc.)
pub struct RelationsRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> RelationsRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Get event relations with optional filtering
    pub async fn get_event_relations(
        &self,
        room_id: &str,
        event_id: &str,
        rel_type: Option<&str>,
        event_type: Option<&str>,
    ) -> Result<RelationsResponse, RepositoryError> {
        let mut query = "
            SELECT event.* FROM event_relations
            INNER JOIN event ON event_relations.child_event_id = event.event_id
            WHERE event_relations.room_id = $room_id
            AND event_relations.parent_event_id = $event_id
        "
        .to_string();

        let mut params = vec![
            ("room_id", room_id.to_string()),
            ("event_id", event_id.to_string()),
        ];

        // Add relation type filter
        if let Some(rel_type_filter) = rel_type {
            query.push_str(" AND event_relations.rel_type = $rel_type");
            params.push(("rel_type", rel_type_filter.to_string()));
        }

        // Add event type filter
        if let Some(event_type_filter) = event_type {
            query.push_str(" AND event.event_type = $event_type");
            params.push(("event_type", event_type_filter.to_string()));
        }

        query.push_str(" ORDER BY event.origin_server_ts ASC");

        let mut result = self.db.query(&query);
        for (key, value) in params {
            result = result.bind((key, value));
        }

        let mut query_result = result.await?;
        let events: Vec<Event> = query_result.take(0)?;

        // Generate pagination tokens
        let limit = 50_usize; // Default limit for relations
        let next_batch = crate::pagination::generate_next_batch(&events, room_id, limit);
        let prev_batch = crate::pagination::generate_prev_batch(&events, room_id, limit);

        Ok(RelationsResponse {
            chunk: events,
            next_batch,
            prev_batch,
        })
    }

    /// Add a relation between two events
    pub async fn add_event_relation(
        &self,
        room_id: &str,
        parent_event_id: &str,
        child_event_id: &str,
        rel_type: &str,
    ) -> Result<(), RepositoryError> {
        // Check if relation already exists
        let existing_query = "
            SELECT id FROM event_relations
            WHERE room_id = $room_id
            AND parent_event_id = $parent_event_id
            AND child_event_id = $child_event_id
            AND rel_type = $rel_type
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(existing_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("parent_event_id", parent_event_id.to_string()))
            .bind(("child_event_id", child_event_id.to_string()))
            .bind(("rel_type", rel_type.to_string()))
            .await?;
        let existing: Vec<serde_json::Value> = result.take(0)?;

        if !existing.is_empty() {
            return Err(RepositoryError::Conflict {
                message: format!(
                    "Relation already exists: {}->{}:{}",
                    parent_event_id, child_event_id, rel_type
                ),
            });
        }

        // Verify both events exist
        let parent_exists_query =
            "SELECT event_id FROM event WHERE event_id = $event_id AND room_id = $room_id LIMIT 1";
        let mut parent_result = self
            .db
            .query(parent_exists_query)
            .bind(("event_id", parent_event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;
        let parent_events: Vec<serde_json::Value> = parent_result.take(0)?;

        if parent_events.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Parent Event".to_string(),
                id: parent_event_id.to_string(),
            });
        }

        let child_exists_query =
            "SELECT event_id FROM event WHERE event_id = $event_id AND room_id = $room_id LIMIT 1";
        let mut child_result = self
            .db
            .query(child_exists_query)
            .bind(("event_id", child_event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;
        let child_events: Vec<serde_json::Value> = child_result.take(0)?;

        if child_events.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Child Event".to_string(),
                id: child_event_id.to_string(),
            });
        }

        // Create the relation
        let relation_id = format!("rel_{}", Uuid::new_v4());
        let insert_query = "
            INSERT INTO event_relations (
                id, room_id, parent_event_id, child_event_id, rel_type, created_at
            ) VALUES (
                $id, $room_id, $parent_event_id, $child_event_id, $rel_type, $created_at
            )
        ";

        self.db
            .query(insert_query)
            .bind(("id", relation_id))
            .bind(("room_id", room_id.to_string()))
            .bind(("parent_event_id", parent_event_id.to_string()))
            .bind(("child_event_id", child_event_id.to_string()))
            .bind(("rel_type", rel_type.to_string()))
            .bind(("created_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Remove a relation between two events
    pub async fn remove_event_relation(
        &self,
        room_id: &str,
        parent_event_id: &str,
        child_event_id: &str,
    ) -> Result<(), RepositoryError> {
        let delete_query = "
            DELETE FROM event_relations
            WHERE room_id = $room_id
            AND parent_event_id = $parent_event_id
            AND child_event_id = $child_event_id
        ";

        self.db
            .query(delete_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("parent_event_id", parent_event_id.to_string()))
            .bind(("child_event_id", child_event_id.to_string()))
            .await?;

        Ok(())
    }

    /// Get related events in specified direction
    pub async fn get_related_events(
        &self,
        room_id: &str,
        event_id: &str,
        direction: RelationDirection,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = match direction {
            RelationDirection::Forward => {
                // Get events that relate to this event (children)
                "
                SELECT event.* FROM event_relations
                INNER JOIN event ON event_relations.child_event_id = event.event_id
                WHERE event_relations.room_id = $room_id
                AND event_relations.parent_event_id = $event_id
                ORDER BY event.origin_server_ts ASC
                "
            },
            RelationDirection::Backward => {
                // Get events that this event relates to (parents)
                "
                SELECT event.* FROM event_relations
                INNER JOIN event ON event_relations.parent_event_id = event.event_id
                WHERE event_relations.room_id = $room_id
                AND event_relations.child_event_id = $event_id
                ORDER BY event.origin_server_ts ASC
                "
            },
            RelationDirection::Both => {
                // Get events in both directions
                "
                SELECT event.* FROM event_relations
                INNER JOIN event ON (
                    event_relations.child_event_id = event.event_id
                    OR event_relations.parent_event_id = event.event_id
                )
                WHERE event_relations.room_id = $room_id
                AND (
                    event_relations.parent_event_id = $event_id
                    OR event_relations.child_event_id = $event_id
                )
                AND event.event_id != $event_id
                ORDER BY event.origin_server_ts ASC
                "
            },
        };

        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let events: Vec<Event> = result.take(0)?;

        Ok(events)
    }

    /// Get aggregated relation statistics for an event
    pub async fn get_aggregated_relations(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<RelationAggregation, RepositoryError> {
        // Get reactions count by emoji
        let reactions_query = "
            SELECT content.relates_to.key as emoji, count() as count
            FROM event_relations
            INNER JOIN event ON event_relations.child_event_id = event.event_id
            WHERE event_relations.room_id = $room_id
            AND event_relations.parent_event_id = $event_id
            AND event.event_type = 'm.reaction'
            GROUP BY content.relates_to.key
        ";
        let mut reactions_result = self
            .db
            .query(reactions_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let reactions_data: Vec<serde_json::Value> = reactions_result.take(0)?;

        let mut reactions = HashMap::new();
        for reaction_data in reactions_data {
            if let (Some(emoji), Some(count)) = (
                reaction_data.get("emoji").and_then(|v| v.as_str()),
                reaction_data.get("count").and_then(|v| v.as_u64()),
            ) {
                reactions.insert(emoji.to_string(), count as u32);
            }
        }

        // Get threads count (events with m.thread relation type)
        let threads_query = "
            SELECT count() as count
            FROM event_relations
            INNER JOIN event ON event_relations.child_event_id = event.event_id
            WHERE event_relations.room_id = $room_id
            AND event_relations.parent_event_id = $event_id
            AND event_relations.rel_type = 'm.thread'
            GROUP ALL
        ";
        let mut threads_result = self
            .db
            .query(threads_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let threads_count: Option<i64> = threads_result.take(0)?;

        // Get replies count (events with m.in_reply_to in content)
        let replies_query = "
            SELECT count() as count
            FROM event
            WHERE event.room_id = $room_id
            AND event.content.\"m.relates_to\".\"m.in_reply_to\".event_id = $event_id
            GROUP ALL
        ";
        let mut replies_result = self
            .db
            .query(replies_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let replies_count: Option<i64> = replies_result.take(0)?;

        // Get edits count
        let edits_query = "
            SELECT count() as count
            FROM event_relations
            INNER JOIN event ON event_relations.child_event_id = event.event_id
            WHERE event_relations.room_id = $room_id
            AND event_relations.parent_event_id = $event_id
            AND event_relations.rel_type = 'm.replace'
            GROUP ALL
        ";
        let mut edits_result = self
            .db
            .query(edits_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let edits_count: Option<i64> = edits_result.take(0)?;

        // Get annotations count
        let annotations_query = "
            SELECT count() as count
            FROM event_relations
            INNER JOIN event ON event_relations.child_event_id = event.event_id
            WHERE event_relations.room_id = $room_id
            AND event_relations.parent_event_id = $event_id
            AND event_relations.rel_type = 'm.annotation'
            GROUP ALL
        ";
        let mut annotations_result = self
            .db
            .query(annotations_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let annotations_count: Option<i64> = annotations_result.take(0)?;

        Ok(RelationAggregation {
            reactions,
            replies: replies_count.unwrap_or(0) as u32,
            edits: edits_count.unwrap_or(0) as u32,
            annotations: annotations_count.unwrap_or(0) as u32,
            threads: threads_count.unwrap_or(0) as u32,
        })
    }

    /// Validate if user has permission to create relations
    pub async fn validate_relation_permissions(
        &self,
        room_id: &str,
        user_id: &str,
        parent_event_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if user is a member of the room
        let membership_query = "
            SELECT membership FROM membership
            WHERE room_id = $room_id AND user_id = $user_id
            AND membership = 'join'
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(membership_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let memberships: Vec<serde_json::Value> = result.take(0)?;

        if memberships.is_empty() {
            return Ok(false);
        }

        // Check if the parent event exists and is accessible
        let event_query = "
            SELECT event_id FROM event
            WHERE room_id = $room_id AND event_id = $event_id
            LIMIT 1
        ";
        let mut event_result = self
            .db
            .query(event_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", parent_event_id.to_string()))
            .await?;
        let events: Vec<serde_json::Value> = event_result.take(0)?;

        Ok(!events.is_empty())
    }

    /// Get relation by ID
    pub async fn get_relation_by_id(
        &self,
        relation_id: &str,
    ) -> Result<Option<EventRelation>, RepositoryError> {
        let query = "SELECT * FROM event_relations WHERE id = $id LIMIT 1";
        let mut result = self.db.query(query).bind(("id", relation_id.to_string())).await?;
        let relations_data: Vec<serde_json::Value> = result.take(0)?;

        if let Some(relation_data) = relations_data.first() {
            let relation = EventRelation {
                id: relation_data.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                room_id: relation_data
                    .get("room_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                parent_event_id: relation_data
                    .get("parent_event_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                child_event_id: relation_data
                    .get("child_event_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                rel_type: relation_data
                    .get("rel_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                created_at: relation_data
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(Utc::now),
            };
            Ok(Some(relation))
        } else {
            Ok(None)
        }
    }

    /// Get all relations for a room (for moderation/admin purposes)
    pub async fn get_room_relations(
        &self,
        room_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<EventRelation>, RepositoryError> {
        let query = match limit {
            Some(l) => {
                format!(
                    "SELECT * FROM event_relations WHERE room_id = $room_id ORDER BY created_at DESC LIMIT {}",
                    l
                )
            },
            None => {
                "SELECT * FROM event_relations WHERE room_id = $room_id ORDER BY created_at DESC"
                    .to_string()
            },
        };

        let mut result = self.db.query(&query).bind(("room_id", room_id.to_string())).await?;
        let relations_data: Vec<serde_json::Value> = result.take(0)?;

        let mut relations = Vec::new();
        for relation_data in relations_data {
            let relation = EventRelation {
                id: relation_data.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                room_id: relation_data
                    .get("room_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                parent_event_id: relation_data
                    .get("parent_event_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                child_event_id: relation_data
                    .get("child_event_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                rel_type: relation_data
                    .get("rel_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                created_at: relation_data
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(Utc::now),
            };
            relations.push(relation);
        }

        Ok(relations)
    }

    /// Cleanup orphaned relations (where parent or child event no longer exists)
    pub async fn cleanup_orphaned_relations(&self, room_id: &str) -> Result<u32, RepositoryError> {
        let cleanup_query = "
            DELETE FROM event_relations
            WHERE room_id = $room_id
            AND (
                parent_event_id NOT IN (SELECT event_id FROM event WHERE room_id = $room_id)
                OR child_event_id NOT IN (SELECT event_id FROM event WHERE room_id = $room_id)
            )
        ";

        let mut result =
            self.db.query(cleanup_query).bind(("room_id", room_id.to_string())).await?;
        let deleted_count: Option<i64> = result.take(0)?;

        Ok(deleted_count.unwrap_or(0) as u32)
    }

    /// Get reaction summary for an event
    pub async fn get_reaction_summary(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<HashMap<String, Vec<String>>, RepositoryError> {
        // Get reactions grouped by emoji with list of users
        let query = "
            SELECT
                event.content.relates_to.key as emoji,
                event.sender as user_id
            FROM event_relations
            INNER JOIN event ON event_relations.child_event_id = event.event_id
            WHERE event_relations.room_id = $room_id
            AND event_relations.parent_event_id = $event_id
            AND event.event_type = 'm.reaction'
            ORDER BY event.origin_server_ts ASC
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let reactions_data: Vec<serde_json::Value> = result.take(0)?;

        let mut reaction_summary: HashMap<String, Vec<String>> = HashMap::new();
        for reaction_data in reactions_data {
            if let (Some(emoji), Some(user_id)) = (
                reaction_data.get("emoji").and_then(|v| v.as_str()),
                reaction_data.get("user_id").and_then(|v| v.as_str()),
            ) {
                reaction_summary
                    .entry(emoji.to_string())
                    .or_default()
                    .push(user_id.to_string());
            }
        }

        Ok(reaction_summary)
    }
}
