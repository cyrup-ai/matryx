use crate::repository::error::RepositoryError;
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct TagsRepository {
    db: Surreal<Any>,
}

impl TagsRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Set a room tag for a user
    pub async fn set_room_tag(
        &self,
        user_id: &str,
        room_id: &str,
        tag: &str,
        content: Option<Value>,
    ) -> Result<(), RepositoryError> {
        // Validate parameters
        if tag.is_empty() {
            return Err(RepositoryError::Validation {
                field: "tag".to_string(),
                message: "Tag cannot be empty".to_string(),
            });
        }

        // Validate tag permissions
        if !self.validate_tag_permissions(user_id, room_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: "User does not have permission to tag this room".to_string(),
            });
        }

        let tag_content = content.unwrap_or(Value::Object(serde_json::Map::new()));

        let query = r#"
            UPDATE room_tags SET
                content = $content,
                updated_at = time::now()
            WHERE user_id = $user_id AND room_id = $room_id AND tag = $tag
            ELSE CREATE room_tags SET
                user_id = $user_id,
                room_id = $room_id,
                tag = $tag,
                content = $content,
                created_at = time::now(),
                updated_at = time::now()
        "#;
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("tag", tag.to_string()))
            .bind(("content", tag_content))
            .await?;

        let _: Vec<Value> = result.take(0)?;
        Ok(())
    }

    /// Get a specific room tag for a user
    pub async fn get_room_tag(
        &self,
        user_id: &str,
        room_id: &str,
        tag: &str,
    ) -> Result<Option<Value>, RepositoryError> {
        let query = "SELECT content FROM room_tags WHERE user_id = $user_id AND room_id = $room_id AND tag = $tag LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("tag", tag.to_string()))
            .await?;

        let tag_rows: Vec<serde_json::Value> = result.take(0)?;
        if let Some(tag_row) = tag_rows.first()
            && let Some(content) = tag_row.get("content") {
            return Ok(Some(content.clone()));
        }

        Ok(None)
    }

    /// Remove a room tag for a user
    pub async fn remove_room_tag(
        &self,
        user_id: &str,
        room_id: &str,
        tag: &str,
    ) -> Result<(), RepositoryError> {
        // Validate tag permissions
        if !self.validate_tag_permissions(user_id, room_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: "User does not have permission to modify tags for this room".to_string(),
            });
        }

        let query =
            "DELETE FROM room_tags WHERE user_id = $user_id AND room_id = $room_id AND tag = $tag";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("tag", tag.to_string()))
            .await?;

        let _: Vec<Value> = result.take(0)?;
        Ok(())
    }

    /// Get all room tags for a user and room
    pub async fn get_room_tags(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> Result<HashMap<String, Value>, RepositoryError> {
        let query =
            "SELECT tag, content FROM room_tags WHERE user_id = $user_id AND room_id = $room_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;

        let tag_rows: Vec<serde_json::Value> = result.take(0)?;
        let mut tags = HashMap::new();

        for row in tag_rows {
            if let (Some(tag), Some(content)) =
                (row.get("tag").and_then(|v| v.as_str()), row.get("content"))
            {
                tags.insert(tag.to_string(), content.clone());
            }
        }

        Ok(tags)
    }

    /// Get all rooms with a specific tag for a user
    pub async fn get_user_tagged_rooms(
        &self,
        user_id: &str,
        tag: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let query = "SELECT room_id FROM room_tags WHERE user_id = $user_id AND tag = $tag";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("tag", tag.to_string()))
            .await?;

        let room_rows: Vec<serde_json::Value> = result.take(0)?;
        let mut room_ids = Vec::new();

        for row in room_rows {
            if let Some(room_id) = row.get("room_id").and_then(|v| v.as_str()) {
                room_ids.push(room_id.to_string());
            }
        }

        Ok(room_ids)
    }

    /// Validate tag permissions for a user and room
    pub async fn validate_tag_permissions(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if user is a member of the room
        let query = "
            SELECT membership FROM membership 
            WHERE room_id = $room_id AND user_id = $user_id 
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let membership_rows: Vec<serde_json::Value> = result.take(0)?;

        if let Some(membership_row) = membership_rows.first()
            && let Some(membership) = membership_row.get("membership").and_then(|v| v.as_str()) {
            // User can tag rooms they have joined
            return Ok(membership == "join");
        }

        // Default to no permission if not a member
        Ok(false)
    }
}
