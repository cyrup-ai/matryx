use crate::repository::error::RepositoryError;
use chrono::Utc;
use matryx_entity::types::Event;
use serde::{Deserialize, Serialize};

use surrealdb::{Connection, Surreal};
use uuid::Uuid;

/// What to include in thread roots response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadInclude {
    /// Include all thread roots
    All,
    /// Include only participated threads
    Participated,
}

/// Response for thread roots query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRootsResponse {
    pub threads: Vec<ThreadRoot>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
}

/// Individual thread root information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRoot {
    pub event_id: String,
    pub latest_event: Event,
    pub unsigned: ThreadUnsigned,
}

/// Unsigned data for thread roots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadUnsigned {
    #[serde(rename = "m.thread")]
    pub thread: ThreadSummary,
}

/// Response for thread list query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadsListResponse {
    pub chunk: Vec<Event>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub prev_batch: Option<String>,
    pub next_batch: Option<String>,
}

/// Thread summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub latest_event: Event,
    pub count: u32,
    pub current_user_participated: bool,
}

/// Thread metadata for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadMetadata {
    pub id: String,
    pub room_id: String,
    pub thread_root_id: String,
    pub latest_event_id: String,
    pub participant_count: u32,
    pub event_count: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// User participation in threads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadParticipation {
    pub id: String,
    pub room_id: String,
    pub thread_root_id: String,
    pub user_id: String,
    pub participating: bool,
    pub last_read_event_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Repository for managing threaded conversations
pub struct ThreadsRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ThreadsRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Get thread roots in a room with optional filtering
    pub async fn get_thread_roots(
        &self,
        room_id: &str,
        user_id: Option<&str>,
        include: Option<ThreadInclude>,
        since: Option<&str>,
        limit: Option<u32>,
    ) -> Result<ThreadRootsResponse, RepositoryError> {
        let mut query = "
            SELECT
                thread_metadata.*,
                event.*
            FROM thread_metadata
            INNER JOIN event ON thread_metadata.latest_event_id = event.event_id
            WHERE thread_metadata.room_id = $room_id
        "
        .to_string();

        let mut params = vec![("room_id", room_id.to_string())];

        // Add include filter
        if let Some(ThreadInclude::Participated) = include {
            if let Some(uid) = user_id {
                // Filter to threads where user has participated
                query.push_str(" AND (
                    thread_metadata.root_event_sender = $user_id OR
                    EXISTS (
                        SELECT 1 FROM thread_event
                        WHERE thread_event.thread_id = thread_metadata.thread_id
                        AND thread_event.sender = $user_id
                    )
                )");
                params.push(("user_id", uid.to_string()));
            } else {
                // Cannot filter by participation without user_id
                tracing::warn!("ThreadInclude::Participated requested but no user_id provided");
            }
        }

        // Add since filter
        if let Some(since_token) = since {
            query.push_str(" AND thread_metadata.updated_at > $since");
            params.push(("since", since_token.to_string()));
        }

        // Add limit
        let limit_value = limit.unwrap_or(20);
        query.push_str(&format!(" ORDER BY thread_metadata.updated_at DESC LIMIT {}", limit_value));

        let mut result = self.db.query(&query);
        for (key, value) in params {
            result = result.bind((key, value));
        }

        let mut query_result = result.await?;
        let thread_data: Vec<serde_json::Value> = query_result.take(0)?;

        let mut threads = Vec::new();
        for data in thread_data {
            // Get the latest event for this thread
            let latest_event_id =
                data.get("latest_event_id").and_then(|v| v.as_str()).unwrap_or("");

            let event_query = "SELECT * FROM event WHERE event_id = $event_id LIMIT 1";
            let mut event_result = self
                .db
                .query(event_query)
                .bind(("event_id", latest_event_id.to_string()))
                .await?;
            let events: Vec<Event> = event_result.take(0)?;

            if let Some(latest_event) = events.into_iter().next() {
                let thread_root_id = data
                    .get("thread_root_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let thread_summary = ThreadSummary {
                    latest_event: latest_event.clone(),
                    count: data.get("event_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    current_user_participated: if let Some(uid) = user_id {
                        self.check_user_participated(room_id, &thread_root_id, uid).await.unwrap_or(false)
                    } else {
                        false
                    },
                };

                let thread_root = ThreadRoot {
                    event_id: thread_root_id,
                    latest_event,
                    unsigned: ThreadUnsigned { thread: thread_summary },
                };

                threads.push(thread_root);
            }
        }

        // Generate pagination tokens based on last thread's timestamp
        let next_batch = if threads.len() as u32 >= limit_value {
            threads.last().map(|t| format!("t_{}", t.latest_event.origin_server_ts))
        } else {
            None
        };

        let prev_batch = if since.is_some() {
            threads.first().map(|t| format!("t_{}", t.latest_event.origin_server_ts))
        } else {
            None  // No prev_batch when at beginning
        };

        Ok(ThreadRootsResponse {
            threads,
            next_batch,
            prev_batch,
        })
    }

    /// Get events in a specific thread
    pub async fn get_thread_events(
        &self,
        room_id: &str,
        thread_root_id: &str,
        since: Option<&str>,
        limit: Option<u32>,
    ) -> Result<ThreadsListResponse, RepositoryError> {
        let mut query = "
            SELECT event.* FROM thread_events
            INNER JOIN event ON thread_events.event_id = event.event_id
            WHERE thread_events.room_id = $room_id
            AND thread_events.thread_root_id = $thread_root_id
        "
        .to_string();

        let mut params = vec![
            ("room_id", room_id.to_string()),
            ("thread_root_id", thread_root_id.to_string()),
        ];

        // Add since filter
        if let Some(since_token) = since {
            // Parse since token as timestamp
            if let Ok(since_ts) = since_token.parse::<i64>() {
                query.push_str(" AND event.origin_server_ts > $since_ts");
                params.push(("since_ts", since_ts.to_string()));
            }
        }

        // Add limit
        let limit_value = limit.unwrap_or(50);
        query.push_str(&format!(" ORDER BY event.origin_server_ts ASC LIMIT {}", limit_value));

        let mut result = self.db.query(&query);
        for (key, value) in params {
            result = result.bind((key, value));
        }

        let mut query_result = result.await?;
        let events: Vec<Event> = query_result.take(0)?;

        // Generate pagination tokens
        let (start, end) = crate::pagination::generate_timeline_tokens(&events, room_id);
        let next_batch = crate::pagination::generate_next_batch(&events, room_id, limit_value as usize);

        Ok(ThreadsListResponse {
            chunk: events,
            start,
            end,
            prev_batch: None, // No prev_batch for initial query
            next_batch,
        })
    }

    /// Add an event to a thread
    pub async fn add_thread_event(
        &self,
        room_id: &str,
        thread_root_id: &str,
        event_id: &str,
    ) -> Result<(), RepositoryError> {
        // Verify the event exists
        let event_query =
            "SELECT * FROM event WHERE room_id = $room_id AND event_id = $event_id LIMIT 1";
        let mut event_result = self
            .db
            .query(event_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let events: Vec<Event> = event_result.take(0)?;

        let event = events.into_iter().next().ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Event".to_string(),
                id: event_id.to_string(),
            }
        })?;

        // Verify thread root exists
        let thread_exists_query = "SELECT id FROM thread_metadata WHERE room_id = $room_id AND thread_root_id = $thread_root_id LIMIT 1";
        let mut thread_result = self
            .db
            .query(thread_exists_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;
        let thread_metadata: Vec<serde_json::Value> = thread_result.take(0)?;

        if thread_metadata.is_empty() {
            // Create thread metadata if it doesn't exist
            self.create_thread_metadata(room_id, thread_root_id, event_id).await?;
        }

        // Check if event is already in thread
        let existing_query = "
            SELECT id FROM thread_events
            WHERE room_id = $room_id
            AND thread_root_id = $thread_root_id
            AND event_id = $event_id
            LIMIT 1
        ";
        let mut existing_result = self
            .db
            .query(existing_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .await?;
        let existing: Vec<serde_json::Value> = existing_result.take(0)?;

        if !existing.is_empty() {
            return Err(RepositoryError::Conflict {
                message: format!("Event {} is already in thread {}", event_id, thread_root_id),
            });
        }

        // Add event to thread
        let thread_event_id = format!("thread_event_{}", Uuid::new_v4());
        let insert_query = "
            INSERT INTO thread_events (
                id, room_id, thread_root_id, event_id, created_at
            ) VALUES (
                $id, $room_id, $thread_root_id, $event_id, $created_at
            )
        ";

        self.db
            .query(insert_query)
            .bind(("id", thread_event_id))
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .bind(("created_at", Utc::now()))
            .await?;

        // Update thread metadata
        self.update_thread_summary(room_id, thread_root_id, event_id, 0).await?;

        // Update user participation
        self.update_user_participation(room_id, thread_root_id, &event.sender, event_id)
            .await?;

        Ok(())
    }

    /// Get thread summary
    pub async fn get_thread_summary(
        &self,
        room_id: &str,
        thread_root_id: &str,
        user_id: Option<&str>,
    ) -> Result<ThreadSummary, RepositoryError> {
        // Get thread metadata
        let metadata_query = "
            SELECT * FROM thread_metadata
            WHERE room_id = $room_id AND thread_root_id = $thread_root_id
            LIMIT 1
        ";
        let mut metadata_result = self
            .db
            .query(metadata_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;
        let metadata: Vec<serde_json::Value> = metadata_result.take(0)?;

        let thread_data = metadata.into_iter().next().ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Thread".to_string(),
                id: thread_root_id.to_string(),
            }
        })?;

        // Get latest event
        let latest_event_id =
            thread_data.get("latest_event_id").and_then(|v| v.as_str()).unwrap_or("");

        let event_query = "SELECT * FROM event WHERE event_id = $event_id LIMIT 1";
        let mut event_result = self
            .db
            .query(event_query)
            .bind(("event_id", latest_event_id.to_string()))
            .await?;
        let events: Vec<Event> = event_result.take(0)?;

        let latest_event = events.into_iter().next().ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Latest Event".to_string(),
                id: latest_event_id.to_string(),
            }
        })?;

        Ok(ThreadSummary {
            latest_event,
            count: thread_data.get("event_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            current_user_participated: if let Some(uid) = user_id {
                self.check_user_participated(room_id, thread_root_id, uid).await.unwrap_or(false)
            } else {
                false
            },
        })
    }

    /// Update thread summary with latest event and participant count
    pub async fn update_thread_summary(
        &self,
        room_id: &str,
        thread_root_id: &str,
        latest_event_id: &str,
        participant_count: u32,
    ) -> Result<(), RepositoryError> {
        // Get current event count
        let count_query = "
            SELECT count() as count FROM thread_events
            WHERE room_id = $room_id AND thread_root_id = $thread_root_id
            GROUP ALL
        ";
        let mut count_result = self
            .db
            .query(count_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;
        let event_count: Option<i64> = count_result.take(0)?;

        // Update thread metadata
        let update_query = "
            UPDATE thread_metadata SET
                latest_event_id = $latest_event_id,
                participant_count = $participant_count,
                event_count = $event_count,
                updated_at = $updated_at
            WHERE room_id = $room_id AND thread_root_id = $thread_root_id
        ";

        self.db
            .query(update_query)
            .bind(("latest_event_id", latest_event_id.to_string()))
            .bind(("participant_count", participant_count))
            .bind(("event_count", event_count.unwrap_or(0) as u32))
            .bind(("updated_at", Utc::now()))
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;

        Ok(())
    }

    /// Validate thread permissions for a user
    pub async fn validate_thread_permissions(
        &self,
        room_id: &str,
        user_id: &str,
        thread_root_id: &str,
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

        // Check if thread root event exists and is accessible
        let event_query = "
            SELECT event_id FROM event
            WHERE room_id = $room_id AND event_id = $thread_root_id
            LIMIT 1
        ";
        let mut event_result = self
            .db
            .query(event_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;
        let events: Vec<serde_json::Value> = event_result.take(0)?;

        Ok(!events.is_empty())
    }

    /// Create thread metadata
    async fn create_thread_metadata(
        &self,
        room_id: &str,
        thread_root_id: &str,
        first_event_id: &str,
    ) -> Result<(), RepositoryError> {
        let metadata_id = format!("thread_{}", Uuid::new_v4());
        let insert_query = "
            INSERT INTO thread_metadata (
                id, room_id, thread_root_id, latest_event_id,
                participant_count, event_count, created_at, updated_at
            ) VALUES (
                $id, $room_id, $thread_root_id, $latest_event_id,
                $participant_count, $event_count, $created_at, $updated_at
            )
        ";

        self.db
            .query(insert_query)
            .bind(("id", metadata_id))
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("latest_event_id", first_event_id.to_string()))
            .bind(("participant_count", 1u32))
            .bind(("event_count", 1u32))
            .bind(("created_at", Utc::now()))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Update user participation in thread
    async fn update_user_participation(
        &self,
        room_id: &str,
        thread_root_id: &str,
        user_id: &str,
        _event_id: &str,
    ) -> Result<(), RepositoryError> {
        // Check if user already participates in this thread
        let existing_query = "
            SELECT id FROM thread_participation
            WHERE room_id = $room_id
            AND thread_root_id = $thread_root_id
            AND user_id = $user_id
            LIMIT 1
        ";
        let mut existing_result = self
            .db
            .query(existing_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let existing: Vec<serde_json::Value> = existing_result.take(0)?;

        if existing.is_empty() {
            // Create new participation record
            let participation_id = format!("participation_{}", Uuid::new_v4());
            let insert_query = "
                INSERT INTO thread_participation (
                    id, room_id, thread_root_id, user_id,
                    participating, created_at, updated_at
                ) VALUES (
                    $id, $room_id, $thread_root_id, $user_id,
                    true, $created_at, $updated_at
                )
            ";

            self.db
                .query(insert_query)
                .bind(("id", participation_id))
                .bind(("room_id", room_id.to_string()))
                .bind(("thread_root_id", thread_root_id.to_string()))
                .bind(("user_id", user_id.to_string()))
                .bind(("created_at", Utc::now()))
                .bind(("updated_at", Utc::now()))
                .await?;
        } else {
            // Update existing participation
            let update_query = "
                UPDATE thread_participation SET
                    participating = true,
                    updated_at = $updated_at
                WHERE room_id = $room_id
                AND thread_root_id = $thread_root_id
                AND user_id = $user_id
            ";

            self.db
                .query(update_query)
                .bind(("updated_at", Utc::now()))
                .bind(("room_id", room_id.to_string()))
                .bind(("thread_root_id", thread_root_id.to_string()))
                .bind(("user_id", user_id.to_string()))
                .await?;
        }

        Ok(())
    }

    /// Check if a user has participated in a thread
    /// Returns true if user has posted to the thread, false otherwise
    async fn check_user_participated(
        &self,
        room_id: &str,
        thread_root_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        let participation = self
            .get_user_participation(room_id, thread_root_id, user_id)
            .await?;
        
        Ok(participation.map(|p| p.participating).unwrap_or(false))
    }

    /// Get user's participation in a thread
    pub async fn get_user_participation(
        &self,
        room_id: &str,
        thread_root_id: &str,
        user_id: &str,
    ) -> Result<Option<ThreadParticipation>, RepositoryError> {
        let query = "
            SELECT * FROM thread_participation
            WHERE room_id = $room_id
            AND thread_root_id = $thread_root_id
            AND user_id = $user_id
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let participation_data: Vec<serde_json::Value> = result.take(0)?;

        if let Some(data) = participation_data.first() {
            let participation = ThreadParticipation {
                id: data.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                room_id: data.get("room_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                thread_root_id: data
                    .get("thread_root_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                user_id: data.get("user_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                participating: data.get("participating").and_then(|v| v.as_bool()).unwrap_or(true),
                last_read_event_id: data.get("last_read_event_id").and_then(|v| v.as_str()).map(String::from),
                created_at: data
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(Utc::now),
                updated_at: data
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(Utc::now),
            };
            Ok(Some(participation))
        } else {
            Ok(None)
        }
    }

    /// Get all participants in a thread
    pub async fn get_thread_participants(
        &self,
        room_id: &str,
        thread_root_id: &str,
    ) -> Result<Vec<ThreadParticipation>, RepositoryError> {
        let query = "
            SELECT * FROM thread_participation
            WHERE room_id = $room_id AND thread_root_id = $thread_root_id
            ORDER BY updated_at DESC
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;
        let participation_data: Vec<serde_json::Value> = result.take(0)?;

        let mut participants = Vec::new();
        for data in participation_data {
            let participation = ThreadParticipation {
                id: data.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                room_id: data.get("room_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                thread_root_id: data
                    .get("thread_root_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                user_id: data.get("user_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                participating: data.get("participating").and_then(|v| v.as_bool()).unwrap_or(true),
                last_read_event_id: data.get("last_read_event_id").and_then(|v| v.as_str()).map(String::from),
                created_at: data
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(Utc::now),
                updated_at: data
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(Utc::now),
            };
            participants.push(participation);
        }

        Ok(participants)
    }

    /// Delete a thread and all its associated data
    pub async fn delete_thread(
        &self,
        room_id: &str,
        thread_root_id: &str,
    ) -> Result<(), RepositoryError> {
        // Delete thread events
        let delete_events_query = "
            DELETE FROM thread_events
            WHERE room_id = $room_id AND thread_root_id = $thread_root_id
        ";
        self.db
            .query(delete_events_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;

        // Delete thread participation
        let delete_participation_query = "
            DELETE FROM thread_participation
            WHERE room_id = $room_id AND thread_root_id = $thread_root_id
        ";
        self.db
            .query(delete_participation_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;

        // Delete thread metadata
        let delete_metadata_query = "
            DELETE FROM thread_metadata
            WHERE room_id = $room_id AND thread_root_id = $thread_root_id
        ";
        self.db
            .query(delete_metadata_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("thread_root_id", thread_root_id.to_string()))
            .await?;

        Ok(())
    }

    /// Cleanup orphaned threads (where root event no longer exists)
    pub async fn cleanup_orphaned_threads(&self, room_id: &str) -> Result<u32, RepositoryError> {
        let cleanup_query = "
            DELETE FROM thread_metadata
            WHERE room_id = $room_id
            AND thread_root_id NOT IN (SELECT event_id FROM event WHERE room_id = $room_id)
        ";

        let mut result =
            self.db.query(cleanup_query).bind(("room_id", room_id.to_string())).await?;
        let deleted_count: Option<i64> = result.take(0)?;

        // Also cleanup orphaned thread events and participation
        let cleanup_events_query = "
            DELETE FROM thread_events
            WHERE room_id = $room_id
            AND thread_root_id NOT IN (SELECT thread_root_id FROM thread_metadata WHERE room_id = $room_id)
        ";
        self.db
            .query(cleanup_events_query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        let cleanup_participation_query = "
            DELETE FROM thread_participation
            WHERE room_id = $room_id
            AND thread_root_id NOT IN (SELECT thread_root_id FROM thread_metadata WHERE room_id = $room_id)
        ";
        self.db
            .query(cleanup_participation_query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        Ok(deleted_count.unwrap_or(0) as u32)
    }
}
