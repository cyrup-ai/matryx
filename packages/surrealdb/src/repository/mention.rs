use crate::repository::error::RepositoryError;
use serde_json::Value;
use surrealdb::{Surreal, engine::any::Any};

pub struct MentionRepository {
    db: Surreal<Any>,
}

impl MentionRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Get room members with display names for mention parsing
    pub async fn get_room_members_for_mentions(
        &self,
        room_id: &str,
    ) -> Result<Vec<(String, String)>, RepositoryError> {
        let query = "
            SELECT user_id, content.displayname
            FROM room_memberships
            WHERE room_id = $room_id AND content.membership = 'join'
        ";

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let members: Vec<Value> = result.take(0)?;

        let mut member_list = Vec::new();
        for member in members {
            if let (Some(user_id), Some(display_name)) = (
                member.get("user_id").and_then(|v| v.as_str()),
                member.get("displayname").and_then(|v| v.as_str()),
            ) && !display_name.is_empty()
            {
                member_list.push((user_id.to_string(), display_name.to_string()));
            }
        }

        Ok(member_list)
    }

    /// Validate that mentioned users are actually room members
    pub async fn validate_mentioned_users(
        &self,
        room_id: &str,
        user_ids: &[String],
    ) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT user_id
            FROM room_memberships
            WHERE room_id = $room_id
            AND user_id IN $user_ids
            AND content.membership = 'join'
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_ids", user_ids.to_vec()))
            .await?;

        let valid_members: Vec<Value> = result.take(0)?;

        let valid_user_ids: Vec<String> = valid_members
            .into_iter()
            .filter_map(|member| {
                member.get("user_id").and_then(|v| v.as_str()).map(|s| s.to_string())
            })
            .collect();

        Ok(valid_user_ids)
    }

    /// Create a mention notification for a specific user
    pub async fn create_mention_notification(
        &self,
        user_id: &str,
        event_id: &str,
        room_id: &str,
        sender: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE mention_notifications SET
                id = rand::uuid(),
                user_id = $user_id,
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                mention_type = 'user',
                created_at = time::now(),
                read = false
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .await?;

        Ok(())
    }

    /// Create room notification for all members (excluding sender)
    pub async fn create_room_notification(
        &self,
        event_id: &str,
        room_id: &str,
        sender: &str,
    ) -> Result<(), RepositoryError> {
        // First get all room members except the sender
        let query = "
            SELECT user_id
            FROM room_memberships
            WHERE room_id = $room_id
            AND content.membership = 'join'
            AND user_id != $sender
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .await?;

        let members: Vec<Value> = result.take(0)?;

        // Create mention notifications for all members
        for member in members {
            if let Some(user_id) = member.get("user_id").and_then(|v| v.as_str()) {
                let query = "
                    CREATE mention_notifications SET
                        id = rand::uuid(),
                        user_id = $user_id,
                        event_id = $event_id,
                        room_id = $room_id,
                        sender = $sender,
                        mention_type = 'room',
                        created_at = time::now(),
                        read = false
                ";

                self.db
                    .query(query)
                    .bind(("user_id", user_id.to_string()))
                    .bind(("event_id", event_id.to_string()))
                    .bind(("room_id", room_id.to_string()))
                    .bind(("sender", sender.to_string()))
                    .await?;
            }
        }

        Ok(())
    }
}
