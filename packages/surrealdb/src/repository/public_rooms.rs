use crate::repository::error::RepositoryError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsResponse {
    pub chunk: Vec<PublicRoomInfo>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomInfo {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: u64,
    pub avatar_url: Option<String>,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: Option<String>,
    pub room_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DirectoryVisibility {
    Public,
    Private,
}

pub struct PublicRoomsRepository {
    db: Surreal<Any>,
}

impl PublicRoomsRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Get public rooms
    pub async fn get_public_rooms(
        &self,
        server_name: Option<&str>,
        limit: Option<u32>,
        since: Option<&str>,
    ) -> Result<PublicRoomsResponse, RepositoryError> {
        let limit = limit.unwrap_or(100).min(1000); // Cap at 1000

        let query = if let Some(_server) = server_name {
            "
            SELECT 
                room_id,
                name,
                topic,
                canonical_alias,
                num_joined_members,
                avatar_url,
                world_readable,
                guest_can_join,
                join_rule,
                room_type,
                updated_at
            FROM public_rooms 
            WHERE server_name = $server_name
            AND visibility = 'public'
            ORDER BY updated_at DESC
            LIMIT $limit
            "
        } else {
            "
            SELECT 
                room_id,
                name,
                topic,
                canonical_alias,
                num_joined_members,
                avatar_url,
                world_readable,
                guest_can_join,
                join_rule,
                room_type,
                updated_at
            FROM public_rooms 
            WHERE visibility = 'public'
            ORDER BY updated_at DESC
            LIMIT $limit
            "
        };

        let mut result = if let Some(server) = server_name {
            self.db
                .query(query)
                .bind(("server_name", server.to_string()))
                .bind(("limit", limit as i64))
                .await?
        } else {
            self.db.query(query).bind(("limit", limit as i64)).await?
        };

        let rooms_data: Vec<serde_json::Value> = result.take(0)?;

        let mut chunk = Vec::new();
        for room_data in rooms_data {
            let room_info = PublicRoomInfo {
                room_id: room_data
                    .get("room_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: room_data.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                topic: room_data.get("topic").and_then(|v| v.as_str()).map(|s| s.to_string()),
                canonical_alias: room_data
                    .get("canonical_alias")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                num_joined_members: room_data
                    .get("num_joined_members")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                avatar_url: room_data
                    .get("avatar_url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                world_readable: room_data
                    .get("world_readable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                guest_can_join: room_data
                    .get("guest_can_join")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                join_rule: room_data
                    .get("join_rule")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                room_type: room_data
                    .get("room_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
            chunk.push(room_info);
        }

        // Generate next_batch token if there might be more results
        let next_batch = if chunk.len() == limit as usize {
            Some(format!("batch_{}", Utc::now().timestamp_millis()))
        } else {
            None
        };

        // Get total count estimate
        let count_query = if let Some(_server) = server_name {
            "SELECT count() FROM public_rooms WHERE server_name = $server_name AND visibility = 'public' GROUP ALL"
        } else {
            "SELECT count() FROM public_rooms WHERE visibility = 'public' GROUP ALL"
        };

        let mut count_result = if let Some(server) = server_name {
            self.db
                .query(count_query)
                .bind(("server_name", server.to_string()))
                .await?
        } else {
            self.db.query(count_query).await?
        };

        let total_count: Option<i64> = count_result.take(0)?;

        Ok(PublicRoomsResponse {
            chunk,
            next_batch,
            prev_batch: since.map(|s| s.to_string()),
            total_room_count_estimate: total_count.map(|c| c as u64),
        })
    }

    /// Add room to directory
    pub async fn add_room_to_directory(
        &self,
        room_id: &str,
        room_info: &PublicRoomInfo,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE public_rooms SET
            room_id = $room_id,
            server_name = $server_name,
            name = $name,
            topic = $topic,
            canonical_alias = $canonical_alias,
            num_joined_members = $num_joined_members,
            avatar_url = $avatar_url,
            world_readable = $world_readable,
            guest_can_join = $guest_can_join,
            join_rule = $join_rule,
            room_type = $room_type,
            visibility = 'public',
            created_at = $created_at,
            updated_at = $updated_at
        ";

        // Extract server name from room ID
        let server_name = if let Some(colon_pos) = room_id.rfind(':') {
            &room_id[colon_pos + 1..]
        } else {
            "localhost"
        };

        self.db
            .query(query)
            .bind(("room_id", room_info.room_id.clone()))
            .bind(("server_name", server_name.to_string()))
            .bind(("name", room_info.name.clone()))
            .bind(("topic", room_info.topic.clone()))
            .bind(("canonical_alias", room_info.canonical_alias.clone()))
            .bind(("num_joined_members", room_info.num_joined_members as i64))
            .bind(("avatar_url", room_info.avatar_url.clone()))
            .bind(("world_readable", room_info.world_readable))
            .bind(("guest_can_join", room_info.guest_can_join))
            .bind(("join_rule", room_info.join_rule.clone()))
            .bind(("room_type", room_info.room_type.clone()))
            .bind(("created_at", Utc::now()))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Remove room from directory
    pub async fn remove_room_from_directory(&self, room_id: &str) -> Result<(), RepositoryError> {
        let query = "DELETE public_rooms WHERE room_id = $room_id";
        self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        Ok(())
    }

    /// Update room directory info
    pub async fn update_room_directory_info(
        &self,
        room_id: &str,
        room_info: &PublicRoomInfo,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE public_rooms SET
            name = $name,
            topic = $topic,
            canonical_alias = $canonical_alias,
            num_joined_members = $num_joined_members,
            avatar_url = $avatar_url,
            world_readable = $world_readable,
            guest_can_join = $guest_can_join,
            join_rule = $join_rule,
            room_type = $room_type,
            updated_at = $updated_at
            WHERE room_id = $room_id
        ";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("name", room_info.name.clone()))
            .bind(("topic", room_info.topic.clone()))
            .bind(("canonical_alias", room_info.canonical_alias.clone()))
            .bind(("num_joined_members", room_info.num_joined_members as i64))
            .bind(("avatar_url", room_info.avatar_url.clone()))
            .bind(("world_readable", room_info.world_readable))
            .bind(("guest_can_join", room_info.guest_can_join))
            .bind(("join_rule", room_info.join_rule.clone()))
            .bind(("room_type", room_info.room_type.clone()))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Check if room is public
    pub async fn is_room_public(&self, room_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() FROM public_rooms 
            WHERE room_id = $room_id AND visibility = 'public'
            GROUP ALL
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Get room directory visibility
    pub async fn get_room_directory_visibility(
        &self,
        room_id: &str,
    ) -> Result<DirectoryVisibility, RepositoryError> {
        let query = "
            SELECT visibility FROM public_rooms 
            WHERE room_id = $room_id
            LIMIT 1
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let visibility_data: Vec<serde_json::Value> = result.take(0)?;

        if let Some(data) = visibility_data.first() {
            if let Some(visibility) = data.get("visibility").and_then(|v| v.as_str()) {
                return match visibility {
                    "public" => Ok(DirectoryVisibility::Public),
                    "private" => Ok(DirectoryVisibility::Private),
                    _ => Ok(DirectoryVisibility::Private),
                };
            }
        }

        Ok(DirectoryVisibility::Private)
    }

    /// Set room directory visibility
    pub async fn set_room_directory_visibility(
        &self,
        room_id: &str,
        visibility: DirectoryVisibility,
    ) -> Result<(), RepositoryError> {
        let visibility_str = match visibility {
            DirectoryVisibility::Public => "public",
            DirectoryVisibility::Private => "private",
        };

        let query = "
            UPDATE public_rooms SET
            visibility = $visibility,
            updated_at = $updated_at
            WHERE room_id = $room_id
        ";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("visibility", visibility_str.to_string()))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Search public rooms
    pub async fn search_public_rooms(
        &self,
        search_term: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PublicRoomInfo>, RepositoryError> {
        let limit = limit.unwrap_or(50).min(500); // Cap at 500

        let query = "
            SELECT 
                room_id,
                name,
                topic,
                canonical_alias,
                num_joined_members,
                avatar_url,
                world_readable,
                guest_can_join,
                join_rule,
                room_type
            FROM public_rooms 
            WHERE visibility = 'public'
            AND (
                name CONTAINS $search_term 
                OR topic CONTAINS $search_term 
                OR canonical_alias CONTAINS $search_term
            )
            ORDER BY num_joined_members DESC
            LIMIT $limit
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("search_term", search_term.to_string()))
            .bind(("limit", limit as i64))
            .await?;

        let rooms_data: Vec<serde_json::Value> = result.take(0)?;

        let mut rooms = Vec::new();
        for room_data in rooms_data {
            let room_info = PublicRoomInfo {
                room_id: room_data
                    .get("room_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: room_data.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                topic: room_data.get("topic").and_then(|v| v.as_str()).map(|s| s.to_string()),
                canonical_alias: room_data
                    .get("canonical_alias")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                num_joined_members: room_data
                    .get("num_joined_members")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                avatar_url: room_data
                    .get("avatar_url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                world_readable: room_data
                    .get("world_readable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                guest_can_join: room_data
                    .get("guest_can_join")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                join_rule: room_data
                    .get("join_rule")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                room_type: room_data
                    .get("room_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
            rooms.push(room_info);
        }

        Ok(rooms)
    }

    /// Update room member count
    pub async fn update_room_member_count(
        &self,
        room_id: &str,
        member_count: u64,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE public_rooms SET
            num_joined_members = $member_count,
            updated_at = $updated_at
            WHERE room_id = $room_id
        ";

        self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("member_count", member_count as i64))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Get public rooms statistics
    pub async fn get_public_rooms_statistics(&self) -> Result<serde_json::Value, RepositoryError> {
        let query = "
            SELECT 
                count() as total_rooms,
                count(CASE WHEN visibility = 'public' THEN 1 END) as public_rooms,
                sum(num_joined_members) as total_members
            FROM public_rooms
            GROUP ALL
        ";

        let mut result = self.db.query(query).await?;
        let stats: Vec<serde_json::Value> = result.take(0)?;
        Ok(stats.into_iter().next().unwrap_or(serde_json::json!({})))
    }
}
