use crate::repository::error::RepositoryError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsResponse {
    pub chunk: Vec<PublicRoomInfo>,
    pub next_token: Option<String>,
    pub prev_token: Option<String>,
    pub total_room_count_estimate: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomInfo {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub alt_aliases: Vec<String>,
    pub num_joined_members: i64,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: Option<String>,
    pub room_type: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoomDirectoryVisibility {
    #[serde(rename = "private")]
    Private,
    #[serde(rename = "public")]
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomAliasInfo {
    pub room_id: String,
    pub servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyProtocol {
    pub user_fields: Vec<String>,
    pub location_fields: Vec<String>,
    pub icon: String,
    pub field_types: HashMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolInstance {
    pub network_id: String,
    pub fields: HashMap<String, String>,
    pub desc: String,
    pub icon: Option<String>,
}

pub struct DirectoryRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> DirectoryRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn get_public_rooms(
        &self,
        server: Option<&str>,
        limit: Option<u32>,
        since: Option<&str>,
    ) -> Result<PublicRoomsResponse, RepositoryError> {
        let limit = limit.unwrap_or(100).min(500); // Cap at 500
        let offset = since.and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);

        let query = if server.is_some() {
            "SELECT room_id, name, topic, canonical_alias, alt_aliases, num_joined_members, world_readable, guest_can_join, join_rule, room_type, avatar_url FROM room WHERE server_name = $server AND directory_visibility = 'public' ORDER BY num_joined_members DESC LIMIT $limit START $offset"
        } else {
            "SELECT room_id, name, topic, canonical_alias, alt_aliases, num_joined_members, world_readable, guest_can_join, join_rule, room_type, avatar_url FROM room WHERE directory_visibility = 'public' ORDER BY num_joined_members DESC LIMIT $limit START $offset"
        };

        let mut result = self.db.query(query).bind(("limit", limit)).bind(("offset", offset));

        if let Some(server) = server {
            result = result.bind(("server", server.to_string()));
        }

        let rooms: Vec<PublicRoomInfo> = result.await?.take(0)?;

        let next_token = if rooms.len() as u32 == limit {
            Some((offset + limit).to_string())
        } else {
            None
        };

        let prev_token = if offset > 0 {
            Some(offset.saturating_sub(limit).to_string())
        } else {
            None
        };

        Ok(PublicRoomsResponse {
            chunk: rooms,
            next_token,
            prev_token,
            total_room_count_estimate: None,
        })
    }

    pub async fn set_room_directory_visibility(
        &self,
        room_id: &str,
        visibility: RoomDirectoryVisibility,
    ) -> Result<(), RepositoryError> {
        let visibility_str = match visibility {
            RoomDirectoryVisibility::Public => "public",
            RoomDirectoryVisibility::Private => "private",
        };

        let query = "UPDATE room SET directory_visibility = $visibility WHERE room_id = $room_id";
        let mut _result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("visibility", visibility_str))
            .await?;

        Ok(())
    }

    pub async fn get_room_directory_visibility(
        &self,
        room_id: &str,
    ) -> Result<RoomDirectoryVisibility, RepositoryError> {
        let query = "SELECT VALUE directory_visibility FROM room WHERE room_id = $room_id LIMIT 1";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let visibility: Option<String> = result.take(0)?;

        match visibility.as_deref() {
            Some("public") => Ok(RoomDirectoryVisibility::Public),
            Some("private") | None => Ok(RoomDirectoryVisibility::Private),
            _ => Ok(RoomDirectoryVisibility::Private),
        }
    }

    pub async fn create_room_alias(
        &self,
        alias: &str,
        room_id: &str,
        creator_id: &str,
    ) -> Result<(), RepositoryError> {
        let alias_data = serde_json::json!({
            "alias": alias,
            "room_id": room_id,
            "creator_id": creator_id,
            "created_at": Utc::now(),
            "servers": [
                "localhost" // Default to local server
            ]
        });

        let _created: Option<serde_json::Value> =
            self.db.create(("room_alias", alias)).content(alias_data).await?;

        Ok(())
    }

    pub async fn get_room_alias(
        &self,
        alias: &str,
    ) -> Result<Option<RoomAliasInfo>, RepositoryError> {
        let query = "SELECT room_id, servers FROM room_alias WHERE alias = $alias LIMIT 1";
        let mut result = self.db.query(query).bind(("alias", alias.to_string())).await?;

        let alias_info: Option<RoomAliasInfo> = result.take(0)?;
        Ok(alias_info)
    }

    pub async fn delete_room_alias(
        &self,
        alias: &str,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        // Check if user has permission to delete this alias
        let has_permission = self.validate_alias_permissions(alias, user_id).await?;
        if !has_permission {
            return Err(RepositoryError::Unauthorized {
                reason: "User does not have permission to delete this alias".to_string(),
            });
        }

        let _deleted: Option<serde_json::Value> = self.db.delete(("room_alias", alias)).await?;

        Ok(())
    }

    pub async fn get_room_aliases(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "SELECT VALUE alias FROM room_alias WHERE room_id = $room_id";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let aliases: Vec<String> = result.take(0)?;
        Ok(aliases)
    }

    pub async fn validate_alias_permissions(
        &self,
        alias: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if user is the creator of the alias
        let query = "SELECT VALUE creator_id FROM room_alias WHERE alias = $alias LIMIT 1";
        let mut result = self.db.query(query).bind(("alias", alias.to_string())).await?;

        let creator_id: Option<String> = result.take(0)?;

        if let Some(creator) = creator_id
            && creator == user_id {
                return Ok(true);
            }

        // Check if user is an admin of the room
        let room_query = "SELECT room_id FROM room_alias WHERE alias = $alias LIMIT 1";
        let mut room_result = self.db.query(room_query).bind(("alias", alias.to_string())).await?;

        let room_id: Option<String> = room_result.take(0)?;

        if let Some(room_id) = room_id {
            let admin_query = "SELECT * FROM membership WHERE room_id = $room_id AND user_id = $user_id AND membership = 'join' LIMIT 1";
            let mut admin_result = self
                .db
                .query(admin_query)
                .bind(("room_id", room_id))
                .bind(("user_id", user_id.to_string()))
                .await?;

            let membership: Option<serde_json::Value> = admin_result.take(0)?;
            return Ok(membership.is_some());
        }

        Ok(false)
    }

    pub async fn get_third_party_protocols(
        &self,
    ) -> Result<Vec<ThirdPartyProtocol>, RepositoryError> {
        // Return empty list for now - third party protocols would be configured
        Ok(vec![])
    }
}
