use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNotice {
    pub notice_id: String,
    pub user_id: String,
    pub notice_type: String,
    pub content: Value,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub is_read: bool,
}

pub struct ServerNoticesRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ServerNoticesRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Create a server notice room for a user
    pub async fn create_server_notice_room(
        &self,
        user_id: &str,
        notice_type: &str,
    ) -> Result<String, RepositoryError> {
        // Check if server notice room already exists for this user
        if let Some(existing_room_id) = self.get_server_notice_room(user_id).await? {
            return Ok(existing_room_id);
        }

        let room_id = format!("!{}", Uuid::new_v4());
        let server_user_id = "@notices:server".to_string(); // Server notices user

        // Create the room
        let room_query = "
            CREATE room SET
                room_id = $room_id,
                creator = $server_user_id,
                room_version = '10',
                is_server_notice_room = true,
                target_user_id = $user_id,
                notice_type = $notice_type,
                created_at = time::now()
        ";

        self.db
            .query(room_query)
            .bind(("room_id", room_id.clone()))
            .bind(("server_user_id", server_user_id.clone()))
            .bind(("user_id", user_id.to_string()))
            .bind(("notice_type", notice_type.to_string()))
            .await?;

        // Create room creation event
        let create_event_id = format!("${}", Uuid::new_v4());
        let timestamp = Utc::now().timestamp_millis();

        let create_event_query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $server_user_id,
                type = 'm.room.create',
                content = {
                    'creator': $server_user_id,
                    'room_version': '10'
                },
                state_key = '',
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";

        self.db
            .query(create_event_query)
            .bind(("event_id", create_event_id))
            .bind(("room_id", room_id.clone()))
            .bind(("server_user_id", server_user_id.clone()))
            .bind(("timestamp", timestamp))
            .await?;

        // Join the server user to the room
        let server_join_event_id = format!("${}", Uuid::new_v4());
        let server_join_query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $server_user_id,
                type = 'm.room.member',
                content = {
                    'membership': 'join',
                    'displayname': 'Server Notices'
                },
                state_key = $server_user_id,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";

        self.db
            .query(server_join_query)
            .bind(("event_id", server_join_event_id))
            .bind(("room_id", room_id.clone()))
            .bind(("server_user_id", server_user_id.clone()))
            .bind(("timestamp", timestamp))
            .await?;

        // Add server membership record
        let server_membership_query = "
            CREATE membership SET
                room_id = $room_id,
                user_id = $server_user_id,
                membership = 'join',
                created_at = time::now()
        ";

        self.db
            .query(server_membership_query)
            .bind(("room_id", room_id.clone()))
            .bind(("server_user_id", server_user_id))
            .await?;

        // Join the target user to the room
        let user_join_event_id = format!("${}", Uuid::new_v4());
        let user_join_query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $user_id,
                type = 'm.room.member',
                content = {
                    'membership': 'join'
                },
                state_key = $user_id,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";

        self.db
            .query(user_join_query)
            .bind(("event_id", user_join_event_id))
            .bind(("room_id", room_id.clone()))
            .bind(("user_id", user_id.to_string()))
            .bind(("timestamp", timestamp))
            .await?;

        // Add user membership record
        let user_membership_query = "
            CREATE membership SET
                room_id = $room_id,
                user_id = $user_id,
                membership = 'join',
                created_at = time::now()
        ";

        self.db
            .query(user_membership_query)
            .bind(("room_id", room_id.clone()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        Ok(room_id)
    }

    /// Get the server notice room for a user
    pub async fn get_server_notice_room(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT room_id FROM room
            WHERE is_server_notice_room = true AND target_user_id = $user_id
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let rooms: Vec<Value> = result.take(0)?;

        if let Some(room_data) = rooms.first() {
            if let Some(room_id) = room_data.get("room_id").and_then(|v| v.as_str()) {
                return Ok(Some(room_id.to_string()));
            }
        }

        Ok(None)
    }

    /// Send a server notice to a user
    pub async fn send_server_notice(
        &self,
        user_id: &str,
        notice_type: &str,
        content: Value,
    ) -> Result<String, RepositoryError> {
        // Get or create server notice room
        let room_id = self.create_server_notice_room(user_id, notice_type).await?;

        let notice_id = format!("${}", Uuid::new_v4());
        let server_user_id = "@notices:server";
        let timestamp = Utc::now().timestamp_millis();

        // Create the notice event
        let notice_event_query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $server_user_id,
                type = 'm.room.message',
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";

        self.db
            .query(notice_event_query)
            .bind(("event_id", notice_id.clone()))
            .bind(("room_id", room_id))
            .bind(("server_user_id", server_user_id.to_string()))
            .bind(("content", content.clone()))
            .bind(("timestamp", timestamp))
            .await?;

        // Store the notice in server_notices table for tracking
        let notice_record_query = "
            CREATE server_notices SET
                notice_id = $notice_id,
                user_id = $user_id,
                notice_type = $notice_type,
                content = $content,
                is_read = false,
                created_at = time::now()
        ";

        self.db
            .query(notice_record_query)
            .bind(("notice_id", notice_id.clone()))
            .bind(("user_id", user_id.to_string()))
            .bind(("notice_type", notice_type.to_string()))
            .bind(("content", content))
            .await?;

        Ok(notice_id)
    }

    /// Get server notices for a user
    pub async fn get_server_notices(
        &self,
        user_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<ServerNotice>, RepositoryError> {
        let limit = limit.unwrap_or(50).min(100); // Cap at 100 notices

        let query = "
            SELECT * FROM server_notices
            WHERE user_id = $user_id
            ORDER BY created_at DESC
            LIMIT $limit
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("limit", limit))
            .await?;

        let notices_data: Vec<Value> = result.take(0)?;

        let mut notices = Vec::new();
        for notice_data in notices_data {
            if let (
                Some(notice_id),
                Some(user_id),
                Some(notice_type),
                Some(content),
                Some(created_at),
                Some(is_read),
            ) = (
                notice_data.get("notice_id").and_then(|v| v.as_str()),
                notice_data.get("user_id").and_then(|v| v.as_str()),
                notice_data.get("notice_type").and_then(|v| v.as_str()),
                notice_data.get("content"),
                notice_data.get("created_at").and_then(|v| v.as_str()),
                notice_data.get("is_read").and_then(|v| v.as_bool()),
            ) {
                let created_at_parsed =
                    created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now());

                let read_at = if is_read {
                    notice_data
                        .get("read_at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                } else {
                    None
                };

                notices.push(ServerNotice {
                    notice_id: notice_id.to_string(),
                    user_id: user_id.to_string(),
                    notice_type: notice_type.to_string(),
                    content: content.clone(),
                    created_at: created_at_parsed,
                    read_at,
                    is_read,
                });
            }
        }

        Ok(notices)
    }

    /// Mark a notice as read
    pub async fn mark_notice_read(
        &self,
        user_id: &str,
        notice_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE server_notices SET
                is_read = true,
                read_at = time::now()
            WHERE notice_id = $notice_id AND user_id = $user_id
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("notice_id", notice_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let updated: Vec<Value> = result.take(0)?;

        if updated.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Server notice".to_string(),
                id: notice_id.to_string(),
            });
        }

        Ok(())
    }

    /// Get count of unread notices for a user
    pub async fn get_unread_notice_count(&self, user_id: &str) -> Result<u32, RepositoryError> {
        let query = "
            SELECT count() as count FROM server_notices
            WHERE user_id = $user_id AND is_read = false
            GROUP ALL
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let counts: Vec<Value> = result.take(0)?;

        if let Some(count_data) = counts.first() {
            if let Some(count) = count_data.get("count").and_then(|v| v.as_u64()) {
                return Ok(count as u32);
            }
        }

        Ok(0)
    }

    /// Clean up old notices
    pub async fn cleanup_old_notices(&self, cutoff: DateTime<Utc>) -> Result<u64, RepositoryError> {
        let query = "
            DELETE FROM server_notices
            WHERE created_at < $cutoff AND is_read = true
        ";

        let mut result = self.db.query(query).bind(("cutoff", cutoff.to_rfc3339())).await?;

        let deleted: Vec<Value> = result.take(0)?;
        Ok(deleted.len() as u64)
    }
}
