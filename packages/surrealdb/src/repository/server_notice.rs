use crate::repository::error::RepositoryError;
use serde_json::Value;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

/// Repository for server notice operations
pub struct ServerNoticeRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ServerNoticeRepository<C> {
    /// Create a new ServerNoticeRepository instance
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Get the server notice room for a user
    pub async fn get_server_notice_room(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT VALUE room_id FROM room_memberships
            WHERE user_id = $user_id 
            AND membership = 'join'
            AND room_id LIKE '!%'
            AND EXISTS (
                SELECT 1 FROM room_memberships rm2 
                WHERE rm2.room_id = room_memberships.room_id 
                AND rm2.user_id LIKE '@server:%'
            )
            LIMIT 1
        ";

        let result: Vec<String> = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?
            .take(0)
            .map_err(RepositoryError::Database)?;

        Ok(result.first().cloned())
    }

    /// Create a new server notices room
    pub async fn create_server_notice_room(
        &self,
        user_id: &str,
        server_name: &str,
    ) -> Result<String, RepositoryError> {
        let room_id = format!("!{}", Uuid::new_v4());
        let server_user_id = format!("@server:{}", server_name);

        let create_room_query = "
            CREATE rooms SET
                room_id = $room_id,
                creator = $creator,
                room_version = '10',
                created_at = time::now()
        ";

        self.db
            .query(create_room_query)
            .bind(("room_id", room_id.clone()))
            .bind(("creator", server_user_id.clone()))
            .await
            .map_err(RepositoryError::Database)?;

        // Add the target user to the room as a member
        let add_member_query = "
            CREATE room_memberships SET
                room_id = $room_id,
                user_id = $user_id,
                membership = 'join',
                created_at = time::now()
        ";

        self.db
            .query(add_member_query)
            .bind(("room_id", room_id.clone()))
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        Ok(room_id)
    }

    /// Send a server notice message to a room
    pub async fn send_server_notice(
        &self,
        user_id: &str,
        room_id: &str,
        notice_content: &Value,
        server_name: &str,
    ) -> Result<String, RepositoryError> {
        // Validate that the user is a member of the room
        let membership_check = "
            SELECT VALUE count() FROM room_memberships
            WHERE room_id = $room_id AND user_id = $user_id AND membership = 'join'
        ";

        let membership_result: Vec<i64> = self
            .db
            .query(membership_check)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?
            .take(0)
            .map_err(RepositoryError::Database)?;

        if membership_result.is_empty() || membership_result[0] == 0 {
            return Err(RepositoryError::NotFound {
                entity_type: "room_membership".to_string(),
                id: format!("{}:{}", room_id, user_id),
            });
        }

        let event_id = format!("${}", Uuid::new_v4());
        let server_user_id = format!("@server:{}", server_name);
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        let query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                type = $type,
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("event_id", event_id.clone()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", server_user_id.clone()))
            .bind(("type", "m.room.message"))
            .bind(("content", notice_content.clone()))
            .bind(("timestamp", timestamp))
            .await
            .map_err(RepositoryError::Database)?;

        Ok(event_id)
    }
}
