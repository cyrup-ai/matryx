use crate::repository::error::RepositoryError;
use matryx_entity::{Event, ThreadSummary};
use serde_json::Value;
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone)]
pub struct ThreadEventsResponse {
    pub events: Vec<Event>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
}

pub struct ThreadRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ThreadRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Get thread events for a specific thread root
    pub async fn get_thread_events(
        &self,
        thread_root_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, RepositoryError> {
        let limit = limit.unwrap_or(50).min(100); // Cap at 100 events

        let query = "
            SELECT e.* FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $thread_root_id
            AND r.rel_type = 'm.thread'
            ORDER BY e.origin_server_ts ASC
            LIMIT $limit
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("limit", limit))
            .await?;

        let events: Vec<Value> = result.take(0)?;

        let mut thread_events = Vec::new();
        for event_data in events {
            if let Ok(event) = self.value_to_event(event_data) {
                thread_events.push(event);
            }
        }

        Ok(thread_events)
    }

    /// Get thread events starting from a specific pagination token
    pub async fn get_thread_events_from(
        &self,
        thread_root_id: &str,
        from_token: &str,
        limit: Option<u32>,
    ) -> Result<ThreadEventsResponse, RepositoryError> {
        let limit = limit.unwrap_or(50).min(100); // Cap at 100 events

        // Parse the from_token as an event ID for pagination
        let query = "
            SELECT e.* FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $thread_root_id
            AND r.rel_type = 'm.thread'
            AND e.origin_server_ts > (
                SELECT origin_server_ts FROM room_timeline_events
                WHERE event_id = $from_token
            )
            ORDER BY e.origin_server_ts ASC
            LIMIT $limit
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("from_token", from_token.to_string()))
            .bind(("limit", limit))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to query thread events from token: {}", e),
                operation: "get_thread_events_from".to_string(),
            })?;

        let events: Vec<Value> = result
            .take(0)
            .map_err(|e| RepositoryError::DatabaseError {
                message: format!("Failed to extract thread events result: {}", e),
                operation: "get_thread_events_from".to_string(),
            })?;

        let mut thread_events = Vec::new();
        for event_data in events {
            if let Ok(event) = self.value_to_event(event_data) {
                thread_events.push(event);
            }
        }

        // Generate pagination tokens
        let next_batch = if thread_events.len() as u32 >= limit {
            thread_events.last().map(|e| format!("e_{}", e.origin_server_ts))
        } else {
            None
        };

        let prev_batch = thread_events.first().map(|e| format!("e_{}", e.origin_server_ts));

        Ok(ThreadEventsResponse {
            events: thread_events,
            next_batch,
            prev_batch,
        })
    }

    /// Create a thread relation between an event and thread root
    pub async fn create_thread_relation(
        &self,
        thread_event_id: &str,
        thread_root_id: &str,
        room_id: &str,
        sender: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE event_relations SET
                event_id = $thread_event_id,
                relates_to_event_id = $thread_root_id,
                rel_type = 'm.thread',
                room_id = $room_id,
                sender = $sender,
                created_at = time::now()
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("thread_event_id", thread_event_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .await?;

        let _: Vec<Value> = result.take(0)?;
        Ok(())
    }

    /// Get unique participants in a thread
    pub async fn get_thread_participants(
        &self,
        thread_root_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT DISTINCT sender FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $thread_root_id
            AND r.rel_type = 'm.thread'
            ORDER BY sender
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;

        let participants: Vec<Value> = result.take(0)?;

        let participant_list: Vec<String> = participants
            .into_iter()
            .filter_map(|p| p.get("sender").and_then(|s| s.as_str()).map(|s| s.to_string()))
            .collect();

        Ok(participant_list)
    }

    /// Get thread summary including count, participants, and latest event
    pub async fn get_thread_summary(
        &self,
        thread_root_id: &str,
    ) -> Result<ThreadSummary, RepositoryError> {
        let thread_events = self.get_thread_events(thread_root_id, Some(50)).await?;

        let latest_event = thread_events.last().cloned();
        let count = thread_events.len();

        // Get unique participants
        let mut participants: Vec<String> = thread_events
            .iter()
            .map(|e| e.sender.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        participants.sort();

        Ok(ThreadSummary {
            latest_event,
            count,
            participated: false, // This will be set by the caller based on user context
            participants,
        })
    }

    /// Update thread metadata/summary
    pub async fn update_thread_summary(
        &self,
        thread_root_id: &str,
        summary: &ThreadSummary,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE thread_metadata SET
                count = $count,
                participants = $participants,
                latest_event_id = $latest_event_id,
                updated_at = time::now()
            WHERE thread_root_id = $thread_root_id
            ELSE CREATE thread_metadata SET
                id = rand::uuid(),
                thread_root_id = $thread_root_id,
                count = $count,
                participants = $participants,
                latest_event_id = $latest_event_id,
                created_at = time::now(),
                updated_at = time::now()
        ";

        let latest_event_id = summary.latest_event.as_ref().map(|e| e.event_id.clone());

        let mut result = self
            .db
            .query(query)
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("count", summary.count as u64))
            .bind((
                "participants",
                serde_json::to_value(&summary.participants).map_err(|e| {
                    RepositoryError::Database(surrealdb::Error::msg(format!(
                        "JSON serialization error: {}",
                        e
                    )))
                })?,
            ))
            .bind(("latest_event_id", latest_event_id))
            .await?;

        let _: Vec<Value> = result.take(0)?;
        Ok(())
    }

    /// Get count of events in a thread (for depth validation)
    pub async fn get_thread_count(&self, thread_root_id: &str) -> Result<u64, RepositoryError> {
        let query = "
            SELECT COUNT(*) as count FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $thread_root_id
            AND r.rel_type = 'm.thread'
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;

        let counts: Vec<Value> = result.take(0)?;

        if let Some(count_data) = counts.first() &&
            let Some(count) = count_data.get("count").and_then(|c| c.as_u64())
        {
            return Ok(count);
        }

        Ok(0)
    }

    /// Get a single event by ID
    pub async fn get_event(&self, event_id: &str) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT event_id, room_id, sender, content, origin_server_ts, type
            FROM room_timeline_events
            WHERE event_id = $event_id
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("event_id", event_id.to_string())).await?;

        let events: Vec<Value> = result.take(0)?;

        if let Some(event_data) = events.first() {
            Ok(Some(self.value_to_event(event_data.clone())?))
        } else {
            Ok(None)
        }
    }

    /// Convert SurrealDB Value to Event entity
    fn value_to_event(&self, event_data: Value) -> Result<Event, RepositoryError> {
        Ok(Event {
            event_id: event_data["event_id"]
                .as_str()
                .ok_or_else(|| {
                    RepositoryError::Database(surrealdb::Error::msg("Missing event_id"))
                })?
                .to_string(),
            room_id: event_data["room_id"]
                .as_str()
                .ok_or_else(|| RepositoryError::Database(surrealdb::Error::msg("Missing room_id")))?
                .to_string(),
            sender: event_data["sender"]
                .as_str()
                .ok_or_else(|| RepositoryError::Database(surrealdb::Error::msg("Missing sender")))?
                .to_string(),
            content: matryx_entity::EventContent::Unknown(event_data["content"].clone()),
            origin_server_ts: event_data["origin_server_ts"].as_u64().ok_or_else(|| {
                RepositoryError::Database(surrealdb::Error::msg("Missing origin_server_ts"))
            })? as i64,
            event_type: event_data["type"]
                .as_str()
                .ok_or_else(|| RepositoryError::Database(surrealdb::Error::msg("Missing type")))?
                .to_string(),
            state_key: None,
            unsigned: None,
            auth_events: None,
            depth: None,
            hashes: None,
            prev_events: None,
            signatures: None,
            soft_failed: None,
            received_ts: None,
            outlier: None,
            redacts: None,
            rejected_reason: None,
        })
    }
}
