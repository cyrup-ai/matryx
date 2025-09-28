use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomAliasInfo {
    pub alias: String,
    pub room_id: String,
    pub creator: String,
    pub created_at: DateTime<Utc>,
    pub servers: Vec<String>,
}

#[derive(Clone)]
pub struct RoomAliasRepository {
    db: Surreal<Any>,
}

impl RoomAliasRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Create a new room alias
    pub async fn create_alias(
        &self,
        alias: &str,
        room_id: &str,
        creator: &str,
    ) -> Result<RoomAliasInfo, RepositoryError> {
        // Validate alias format
        if !self.validate_alias_format(alias).await? {
            return Err(RepositoryError::Validation {
                field: "alias".to_string(),
                message: format!("Invalid alias format: {}", alias),
            });
        }

        // Check if alias already exists
        if self.alias_exists(alias).await? {
            return Err(RepositoryError::Conflict {
                message: format!("Room alias '{}' already exists", alias),
            });
        }

        // Check if user can create this alias
        if !self.can_create_alias(alias, creator).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} not authorized to create alias {}", creator, alias),
            });
        }

        // Extract server name from alias
        let server_name = if let Some(colon_pos) = alias.rfind(':') {
            alias[colon_pos + 1..].to_string()
        } else {
            return Err(RepositoryError::Validation {
                field: "alias".to_string(),
                message: "Alias must contain server name".to_string(),
            });
        };

        let alias_info = RoomAliasInfo {
            alias: alias.to_string(),
            room_id: room_id.to_string(),
            creator: creator.to_string(),
            created_at: Utc::now(),
            servers: vec![server_name],
        };

        // Insert into database
        let query = "
            INSERT INTO room_aliases (
                alias, room_id, creator, created_at, servers
            ) VALUES (
                $alias, $room_id, $creator, $created_at, $servers
            )
        ";

        self.db
            .query(query)
            .bind(("alias", alias_info.alias.clone()))
            .bind(("room_id", alias_info.room_id.clone()))
            .bind(("creator", alias_info.creator.clone()))
            .bind(("created_at", alias_info.created_at))
            .bind(("servers", alias_info.servers.clone()))
            .await?;

        Ok(alias_info)
    }

    /// Delete a room alias
    pub async fn delete_alias(&self, alias: &str) -> Result<(), RepositoryError> {
        let query = "DELETE FROM room_aliases WHERE alias = $alias";
        let mut result = self.db.query(query).bind(("alias", alias.to_string())).await?;

        // Check if alias existed
        let deleted: Option<serde_json::Value> = result.take(0)?;
        if deleted.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room alias".to_string(),
                id: alias.to_string(),
            });
        }

        Ok(())
    }

    /// Resolve alias to room information
    pub async fn resolve_alias(
        &self,
        alias: &str,
    ) -> Result<Option<RoomAliasInfo>, RepositoryError> {
        let query = "
            SELECT alias, room_id, creator, created_at, servers
            FROM room_aliases 
            WHERE alias = $alias 
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("alias", alias.to_string())).await?;
        let aliases: Vec<RoomAliasInfo> = result.take(0)?;

        Ok(aliases.into_iter().next())
    }

    /// Get all aliases for a room
    pub async fn get_room_aliases(
        &self,
        room_id: &str,
    ) -> Result<Vec<RoomAliasInfo>, RepositoryError> {
        let query = "
            SELECT alias, room_id, creator, created_at, servers
            FROM room_aliases 
            WHERE room_id = $room_id
            ORDER BY created_at ASC
        ";

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let aliases: Vec<RoomAliasInfo> = result.take(0)?;

        Ok(aliases)
    }

    /// Check if alias exists
    pub async fn alias_exists(&self, alias: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT count() FROM room_aliases WHERE alias = $alias GROUP ALL";

        let mut result = self.db.query(query).bind(("alias", alias.to_string())).await?;
        let count: Option<i64> = result.take(0)?;

        Ok(count.unwrap_or(0) > 0)
    }

    /// Validate alias format according to Matrix specification
    pub async fn validate_alias_format(&self, alias: &str) -> Result<bool, RepositoryError> {
        // Matrix alias format: #localpart:server_name
        // Must start with #, contain exactly one :, and have valid characters

        if !alias.starts_with('#') {
            return Ok(false);
        }

        let colon_count = alias.matches(':').count();
        if colon_count != 1 {
            return Ok(false);
        }

        let colon_pos = alias.rfind(':').unwrap(); // Safe because we checked count above

        // Check localpart (between # and :)
        let localpart = &alias[1..colon_pos];
        if localpart.is_empty() {
            return Ok(false);
        }

        // Check server name (after :)
        let server_name = &alias[colon_pos + 1..];
        if server_name.is_empty() {
            return Ok(false);
        }

        // Basic character validation (simplified)
        // Full Matrix spec has more complex rules for allowed characters
        if localpart.chars().all(|c| c.is_alphanumeric() || "_.-".contains(c)) &&
            server_name.chars().all(|c| c.is_alphanumeric() || "_.-:".contains(c))
        {
            return Ok(true);
        }

        Ok(false)
    }

    /// Check if user can create this alias (simplified implementation)
    pub async fn can_create_alias(
        &self,
        alias: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Extract server name from alias
        let server_name = if let Some(colon_pos) = alias.rfind(':') {
            &alias[colon_pos + 1..]
        } else {
            return Ok(false);
        };

        // Extract user's server from their ID
        let user_server = if let Some(colon_pos) = user_id.rfind(':') {
            &user_id[colon_pos + 1..]
        } else {
            return Ok(false);
        };

        // Basic rule: users can only create aliases on their own server
        // In a full implementation, this would check server ACLs and delegation
        Ok(server_name == user_server)
    }

    /// Get the creator of an alias
    pub async fn get_alias_creator(&self, alias: &str) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT creator FROM room_aliases WHERE alias = $alias LIMIT 1";

        let mut result = self.db.query(query).bind(("alias", alias.to_string())).await?;
        let creators: Vec<serde_json::Value> = result.take(0)?;

        if let Some(creator_record) = creators.first()
            && let Some(creator) = creator_record.get("creator").and_then(|v| v.as_str()) {
                return Ok(Some(creator.to_string()));
            }

        Ok(None)
    }

    /// Get canonical alias for a room
    pub async fn get_canonical_alias(
        &self,
        room_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        // Get the canonical alias from room state
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.canonical_alias'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content")
            && let Some(canonical_alias) = content.get("alias").and_then(|v| v.as_str()) {
                return Ok(Some(canonical_alias.to_string()));
            }

        Ok(None)
    }

    /// Set canonical alias for a room (helper method)
    pub async fn set_canonical_alias(
        &self,
        room_id: &str,
        alias: Option<&str>,
        sender: &str,
    ) -> Result<(), RepositoryError> {
        // Verify alias exists if provided
        if let Some(alias_str) = alias {
            if !self.alias_exists(alias_str).await? {
                return Err(RepositoryError::NotFound {
                    entity_type: "Room alias".to_string(),
                    id: alias_str.to_string(),
                });
            }

            // Verify alias points to this room
            if let Some(alias_info) = self.resolve_alias(alias_str).await?
                && alias_info.room_id != room_id {
                    return Err(RepositoryError::Validation {
                        field: "alias".to_string(),
                        message: "Alias does not point to this room".to_string(),
                    });
                }
        }

        // Create canonical alias event
        let content = if let Some(alias_str) = alias {
            serde_json::json!({
                "alias": alias_str
            })
        } else {
            serde_json::json!({})
        };

        let event_id = format!("${}:{}", uuid::Uuid::new_v4(), "localhost");
        let timestamp = Utc::now();

        let event_query = "
            INSERT INTO event (
                event_id, room_id, sender, event_type, state_key, content,
                origin_server_ts, unsigned, redacts, auth_events, prev_events,
                depth, created_at
            ) VALUES (
                $event_id, $room_id, $sender, 'm.room.canonical_alias', '',
                $content, $timestamp, {}, NONE, [], [], 1, $timestamp
            )
        ";

        self.db
            .query(event_query)
            .bind(("event_id", event_id))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .bind(("content", content))
            .bind(("timestamp", timestamp.timestamp_millis()))
            .await?;

        Ok(())
    }

    /// Get alternative aliases for a room
    pub async fn get_alternative_aliases(
        &self,
        room_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        // Get alternative aliases from room state
        let query = "
            SELECT content FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.canonical_alias'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event) = events.first()
            && let Some(content) = event.get("content")
            && let Some(alt_aliases) = content.get("alt_aliases").and_then(|v| v.as_array()) {
                let aliases: Vec<String> = alt_aliases
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                return Ok(aliases);
            }

        Ok(Vec::new())
    }

    /// Update alternative aliases for a room
    pub async fn set_alternative_aliases(
        &self,
        room_id: &str,
        alt_aliases: &[String],
        sender: &str,
    ) -> Result<(), RepositoryError> {
        // Verify all aliases exist and point to this room
        for alias in alt_aliases {
            if !self.alias_exists(alias).await? {
                return Err(RepositoryError::NotFound {
                    entity_type: "Room alias".to_string(),
                    id: alias.to_string(),
                });
            }

            if let Some(alias_info) = self.resolve_alias(alias).await?
                && alias_info.room_id != room_id {
                    return Err(RepositoryError::Validation {
                        field: "alias".to_string(),
                        message: format!("Alias {} does not point to this room", alias),
                    });
                }
        }

        // Get current canonical alias to preserve it
        let canonical_alias = self.get_canonical_alias(room_id).await?;

        // Create canonical alias event with alternative aliases
        let mut content = serde_json::json!({
            "alt_aliases": alt_aliases
        });

        if let Some(canonical) = canonical_alias {
            content["alias"] = serde_json::Value::String(canonical);
        }

        let event_id = format!("${}:{}", uuid::Uuid::new_v4(), "localhost");
        let timestamp = Utc::now();

        let event_query = "
            INSERT INTO event (
                event_id, room_id, sender, event_type, state_key, content,
                origin_server_ts, unsigned, redacts, auth_events, prev_events,
                depth, created_at
            ) VALUES (
                $event_id, $room_id, $sender, 'm.room.canonical_alias', '',
                $content, $timestamp, {}, NONE, [], [], 1, $timestamp
            )
        ";

        self.db
            .query(event_query)
            .bind(("event_id", event_id))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .bind(("content", content))
            .bind(("timestamp", timestamp.timestamp_millis()))
            .await?;

        Ok(())
    }

    /// Cache alias resolution result
    pub async fn cache_alias_resolution(
        &self,
        alias: &str,
        room_id: &str,
        ttl_seconds: u64,
    ) -> Result<(), RepositoryError> {
        let cache_key = format!("alias_resolution:{}", alias);
        let cache_value = serde_json::json!({
            "room_id": room_id,
            "cached_at": chrono::Utc::now().timestamp(),
            "ttl": ttl_seconds
        });

        let query = "
            INSERT INTO alias_cache (cache_key, cache_value, expires_at) 
            VALUES ($key, $value, time::now() + duration::from_secs($ttl))
            ON DUPLICATE KEY UPDATE 
            cache_value = $value, expires_at = time::now() + duration::from_secs($ttl)
        ";

        self.db
            .query(query)
            .bind(("key", cache_key))
            .bind(("value", cache_value))
            .bind(("ttl", ttl_seconds as i64))
            .await?;

        Ok(())
    }

    /// Check cache for existing alias resolution
    pub async fn get_cached_alias_resolution(&self, alias: &str) -> Result<Option<String>, RepositoryError> {
        let cache_key = format!("alias_resolution:{}", alias);
        let query = "
            SELECT cache_value FROM alias_cache 
            WHERE cache_key = $key AND expires_at > time::now()
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("key", cache_key))
            .await?;

        let cache_records: Vec<serde_json::Value> = result.take(0)?;

        if let Some(cache_record) = cache_records.first()
            && let Some(cache_value) = cache_record.get("cache_value")
            && let Some(room_id) = cache_value.get("room_id").and_then(|v| v.as_str())
        {
            return Ok(Some(room_id.to_string()));
        }

        Ok(None)
    }
}
