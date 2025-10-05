use crate::repository::error::RepositoryError;
use serde::{Deserialize, Serialize};
use surrealdb::{Surreal, engine::any::Any};

// TASK15 SUBTASK 9: Add supporting types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsResponse {
    pub chunk: Vec<PublicRoomEntry>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomEntry {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: u32,
    pub avatar_url: Option<String>,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: String,
    pub room_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsFilter {
    pub limit: Option<u32>,
    pub since: Option<String>,
    pub server: Option<String>,
    pub include_all_known_networks: Option<bool>,
    pub third_party_instance_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoomDirectoryVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomDirectoryInfo {
    pub name: Option<String>,
    pub topic: Option<String>,
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub join_rule: String,
    pub room_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomInfo {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: u32,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: String,
    pub room_type: Option<String>,
    pub visibility: RoomDirectoryVisibility,
}

// TASK15 SUBTASK 2: Create PublicRoomsRepository
pub struct PublicRoomsRepository {
    db: Surreal<Any>,
}

impl PublicRoomsRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Get public rooms with pagination
    pub async fn get_public_rooms(&self, limit: Option<u32>, since: Option<&str>) -> Result<PublicRoomsResponse, RepositoryError> {
        let limit = limit.unwrap_or(10).min(100);
        let offset = self.parse_pagination_token(since).unwrap_or(0);

        let query = r#"
            SELECT room_id, name, topic, canonical_alias, avatar_url, 
                   world_readable, guest_can_join, join_rule, room_type,
                   (SELECT count() FROM membership WHERE room_id = $parent.room_id AND membership = 'join') as num_joined_members
            FROM room 
            WHERE visibility = 'public'
            ORDER BY num_joined_members DESC
            LIMIT $limit START $offset
        "#;

        let mut response = self.db
            .query(query)
            .bind(("limit", limit + 1)) // Get one extra to check for next page
            .bind(("offset", offset))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_public_rooms".to_string(),
            })?;

        let rooms: Vec<PublicRoomEntry> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_public_rooms_parse".to_string(),
        })?;

        let has_more = rooms.len() > limit as usize;
        let chunk = if has_more {
            rooms.into_iter().take(limit as usize).collect()
        } else {
            rooms
        };

        let next_batch = if has_more {
            Some(self.generate_pagination_token(offset + limit))
        } else {
            None
        };

        let prev_batch = if offset > 0 {
            Some(self.generate_pagination_token(offset.saturating_sub(limit)))
        } else {
            None
        };

        let total_count = self.get_public_rooms_count().await?;

        Ok(PublicRoomsResponse {
            chunk,
            next_batch,
            prev_batch,
            total_room_count_estimate: Some(total_count),
        })
    }

    /// Search public rooms with a search term
    pub async fn search_public_rooms(&self, search_term: &str, limit: Option<u32>) -> Result<PublicRoomsResponse, RepositoryError> {
        let limit = limit.unwrap_or(10).min(100);

        let query = r#"
            SELECT room_id, name, topic, canonical_alias, avatar_url, 
                   world_readable, guest_can_join, join_rule, room_type,
                   (SELECT count() FROM membership WHERE room_id = $parent.room_id AND membership = 'join') as num_joined_members
            FROM room 
            WHERE visibility = 'public' 
            AND (name CONTAINS $search_term OR topic CONTAINS $search_term OR canonical_alias CONTAINS $search_term)
            ORDER BY num_joined_members DESC
            LIMIT $limit
        "#;

        let mut response = self.db
            .query(query)
            .bind(("search_term", search_term.to_string()))
            .bind(("limit", limit))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "search_public_rooms".to_string(),
            })?;

        let chunk: Vec<PublicRoomEntry> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "search_public_rooms_parse".to_string(),
        })?;

        Ok(PublicRoomsResponse {
            chunk,
            next_batch: None,
            prev_batch: None,
            total_room_count_estimate: None,
        })
    }

    /// Get total count of public rooms
    pub async fn get_public_rooms_count(&self) -> Result<u64, RepositoryError> {
        let query = "SELECT count() FROM room WHERE visibility = 'public' GROUP ALL";

        let mut response = self.db
            .query(query)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_public_rooms_count".to_string(),
            })?;

        let count: Option<i64> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_public_rooms_count_parse".to_string(),
        })?;

        Ok(count.unwrap_or(0) as u64)
    }

    /// Add room to public directory
    pub async fn add_room_to_directory(&self, room_id: &str, visibility: RoomDirectoryVisibility) -> Result<(), RepositoryError> {
        let visibility_str = match visibility {
            RoomDirectoryVisibility::Public => "public",
            RoomDirectoryVisibility::Private => "private",
        };

        let query = "UPDATE room SET visibility = $visibility WHERE room_id = $room_id";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("visibility", visibility_str.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "add_room_to_directory".to_string(),
            })?;

        Ok(())
    }

    /// Remove room from public directory
    pub async fn remove_room_from_directory(&self, room_id: &str) -> Result<(), RepositoryError> {
        let query = "UPDATE room SET visibility = 'private' WHERE room_id = $room_id";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "remove_room_from_directory".to_string(),
            })?;

        Ok(())
    }

    /// Get room directory visibility
    pub async fn get_room_directory_visibility(&self, room_id: &str) -> Result<Option<RoomDirectoryVisibility>, RepositoryError> {
        let query = "SELECT visibility FROM room WHERE room_id = $room_id";

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_directory_visibility".to_string(),
            })?;

        let visibility: Option<String> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_directory_visibility_parse".to_string(),
        })?;

        match visibility.as_deref() {
            Some("public") => Ok(Some(RoomDirectoryVisibility::Public)),
            Some("private") => Ok(Some(RoomDirectoryVisibility::Private)),
            _ => Ok(None),
        }
    }

    /// Update room directory information
    pub async fn update_room_directory_info(&self, room_id: &str, info: &RoomDirectoryInfo) -> Result<(), RepositoryError> {
        let query = r#"
            UPDATE room SET 
                name = $name,
                topic = $topic,
                avatar_url = $avatar_url,
                canonical_alias = $canonical_alias,
                join_rule = $join_rule,
                room_type = $room_type
            WHERE room_id = $room_id
        "#;

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("name", info.name.clone()))
            .bind(("topic", info.topic.clone()))
            .bind(("avatar_url", info.avatar_url.clone()))
            .bind(("canonical_alias", info.canonical_alias.clone()))
            .bind(("join_rule", info.join_rule.clone()))
            .bind(("room_type", info.room_type.clone()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "update_room_directory_info".to_string(),
            })?;

        Ok(())
    }

    /// Get federation public rooms for a specific server
    pub async fn get_federation_public_rooms(&self, server_name: &str, limit: Option<u32>) -> Result<PublicRoomsResponse, RepositoryError> {
        let limit = limit.unwrap_or(100).min(500);

        let query = r#"
            SELECT room_id, name, topic, canonical_alias, avatar_url, 
                   world_readable, guest_can_join, join_rule, room_type,
                   (SELECT count() FROM membership WHERE room_id = $parent.room_id AND membership = 'join') as num_joined_members
            FROM room 
            WHERE visibility = 'public' AND server_name = $server_name
            ORDER BY num_joined_members DESC
            LIMIT $limit
        "#;

        let mut response = self.db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("limit", limit))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_federation_public_rooms".to_string(),
            })?;

        let chunk: Vec<PublicRoomEntry> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_federation_public_rooms_parse".to_string(),
        })?;

        Ok(PublicRoomsResponse {
            chunk,
            next_batch: None,
            prev_batch: None,
            total_room_count_estimate: None,
        })
    }

    // Helper methods
    fn parse_pagination_token(&self, token: Option<&str>) -> Option<u32> {
        token?.parse().ok()
    }

    fn generate_pagination_token(&self, offset: u32) -> String {
        offset.to_string()
    }

    /// Update room search index for full-text search
    pub async fn update_room_search_index(
        &self,
        room_id: &str,
        name: &str,
        topic: &str,
    ) -> Result<(), RepositoryError> {
        // Update search index fields in the room table
        // This enables full-text search on room name and topic
        let query = "
            UPDATE room
            SET search_name = $search_name,
                search_topic = $search_topic,
                search_updated_at = time::now()
            WHERE room_id = $room_id
        ";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("search_name", name.to_lowercase()))
            .bind(("search_topic", topic.to_lowercase()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "update_room_search_index".to_string(),
            })?;

        Ok(())
    }

    /// Emit directory update event for LiveQuery subscribers
    pub async fn emit_directory_update_event(&self, room_id: &str) -> Result<(), RepositoryError> {
        // Create a directory update event that LiveQuery can pick up
        // This notifies subscribers that the room directory has changed
        let query = "
            CREATE directory_update_event CONTENT {
                room_id: $room_id,
                event_type: 'directory_update',
                timestamp: time::now()
            }
        ";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "emit_directory_update_event".to_string(),
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    include!("public_rooms_tests.rs");
}