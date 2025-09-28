use crate::repository::error::RepositoryError;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeyBackupVersion {
    pub version: String,
    pub algorithm: String,
    pub auth_data: Value,
    pub count: u64,
    pub etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeyBackupData {
    pub first_message_index: u32,
    pub forwarded_count: u32,
    pub is_verified: bool,
    pub session_data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysResponse {
    pub rooms: HashMap<String, RoomKeys>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeys {
    pub sessions: HashMap<String, RoomKeyBackupData>,
}

pub struct RoomKeysRepository {
    db: Surreal<Any>,
}

impl RoomKeysRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Create a new backup version for a user
    pub async fn create_backup_version(
        &self,
        user_id: &str,
        algorithm: &str,
        auth_data: Value,
    ) -> Result<String, RepositoryError> {
        // Validate algorithm
        if algorithm.is_empty() {
            return Err(RepositoryError::Validation {
                field: "algorithm".to_string(),
                message: "Algorithm cannot be empty".to_string(),
            });
        }

        // Get the next version number
        let next_version = self.get_next_version_number(user_id).await?;
        let version = next_version.to_string();
        let etag = format!("etag_{}", Uuid::new_v4());

        // Create the backup version
        let create_query = "
            CREATE room_key_backup_versions SET
                user_id = $user_id,
                version = $version,
                algorithm = $algorithm,
                auth_data = $auth_data,
                count = 0,
                etag = $etag,
                created_at = time::now(),
                updated_at = time::now()
        ";

        self.db
            .query(create_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.clone()))
            .bind(("algorithm", algorithm.to_string()))
            .bind(("auth_data", auth_data))
            .bind(("etag", etag))
            .await?;

        Ok(version)
    }

    /// Get backup version for a user (latest if version not specified)
    pub async fn get_backup_version(
        &self,
        user_id: &str,
        version: Option<&str>,
    ) -> Result<Option<RoomKeyBackupVersion>, RepositoryError> {
        let query = if version.is_some() {
            "
            SELECT * FROM room_key_backup_versions
            WHERE user_id = $user_id AND version = $version
            LIMIT 1
            "
        } else {
            "
            SELECT * FROM room_key_backup_versions
            WHERE user_id = $user_id
            ORDER BY created_at DESC
            LIMIT 1
            "
        };

        let mut query_builder = self.db.query(query).bind(("user_id", user_id.to_string()));

        if let Some(v) = version {
            query_builder = query_builder.bind(("version", v.to_string()));
        }

        let mut result = query_builder.await?;
        let versions_data: Vec<Value> = result.take(0)?;

        if let Some(version_data) = versions_data.first() {
            self.value_to_backup_version(version_data.clone())
        } else {
            Ok(None)
        }
    }

    /// Update an existing backup version
    pub async fn update_backup_version(
        &self,
        user_id: &str,
        version: &str,
        algorithm: &str,
        auth_data: Value,
    ) -> Result<(), RepositoryError> {
        // Verify the version exists and belongs to the user
        if !self.validate_backup_access(user_id, version).await? {
            return Err(RepositoryError::NotFound {
                entity_type: "Backup version".to_string(),
                id: format!("{}:{}", user_id, version),
            });
        }

        let new_etag = format!("etag_{}", Uuid::new_v4());

        let update_query = "
            UPDATE room_key_backup_versions SET
                algorithm = $algorithm,
                auth_data = $auth_data,
                etag = $etag,
                updated_at = time::now()
            WHERE user_id = $user_id AND version = $version
        ";

        let mut result = self
            .db
            .query(update_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .bind(("algorithm", algorithm.to_string()))
            .bind(("auth_data", auth_data))
            .bind(("etag", new_etag))
            .await?;

        let updated: Vec<Value> = result.take(0)?;

        if updated.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Backup version".to_string(),
                id: format!("{}:{}", user_id, version),
            });
        }

        Ok(())
    }

    /// Delete a backup version and all associated room keys
    pub async fn delete_backup_version(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<(), RepositoryError> {
        // Verify the version exists and belongs to the user
        if !self.validate_backup_access(user_id, version).await? {
            return Err(RepositoryError::NotFound {
                entity_type: "Backup version".to_string(),
                id: format!("{}:{}", user_id, version),
            });
        }

        // Delete all room keys for this version
        let delete_keys_query = "
            DELETE FROM room_key_backups
            WHERE user_id = $user_id AND version = $version
        ";

        self.db
            .query(delete_keys_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        // Delete the backup version
        let delete_version_query = "
            DELETE FROM room_key_backup_versions
            WHERE user_id = $user_id AND version = $version
        ";

        self.db
            .query(delete_version_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        Ok(())
    }

    /// Store room keys for backup
    pub async fn store_room_keys(
        &self,
        user_id: &str,
        version: &str,
        room_id: &str,
        session_id: &str,
        key_data: &RoomKeyBackupData,
    ) -> Result<(), RepositoryError> {
        // Verify the version exists and belongs to the user
        if !self.validate_backup_access(user_id, version).await? {
            return Err(RepositoryError::NotFound {
                entity_type: "Backup version".to_string(),
                id: format!("{}:{}", user_id, version),
            });
        }

        // Store or update the room key
        let upsert_query = "
            UPDATE room_key_backups SET
                first_message_index = $first_message_index,
                forwarded_count = $forwarded_count,
                is_verified = $is_verified,
                session_data = $session_data,
                updated_at = time::now()
            WHERE user_id = $user_id AND version = $version 
            AND room_id = $room_id AND session_id = $session_id
            ELSE CREATE room_key_backups SET
                user_id = $user_id,
                version = $version,
                room_id = $room_id,
                session_id = $session_id,
                first_message_index = $first_message_index,
                forwarded_count = $forwarded_count,
                is_verified = $is_verified,
                session_data = $session_data,
                created_at = time::now(),
                updated_at = time::now()
        ";

        self.db
            .query(upsert_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("session_id", session_id.to_string()))
            .bind(("first_message_index", key_data.first_message_index))
            .bind(("forwarded_count", key_data.forwarded_count))
            .bind(("is_verified", key_data.is_verified))
            .bind(("session_data", key_data.session_data.clone()))
            .await?;

        // Update the backup version count and etag
        self.update_backup_count_and_etag(user_id, version).await?;

        Ok(())
    }

    /// Get room keys from backup (optionally filtered by room/session)
    pub async fn get_room_keys(
        &self,
        user_id: &str,
        version: &str,
        room_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<RoomKeysResponse, RepositoryError> {
        // Verify the version exists and belongs to the user
        if !self.validate_backup_access(user_id, version).await? {
            return Err(RepositoryError::NotFound {
                entity_type: "Backup version".to_string(),
                id: format!("{}:{}", user_id, version),
            });
        }

        let mut query = "
            SELECT * FROM room_key_backups
            WHERE user_id = $user_id AND version = $version
        "
        .to_string();

        let mut query_builder = self
            .db
            .query(&query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()));

        // Add room filter if specified
        if let Some(room_id) = room_id {
            query.push_str(" AND room_id = $room_id");
            query_builder = query_builder.bind(("room_id", room_id.to_string()));

            // Add session filter if both room and session specified
            if let Some(session_id) = session_id {
                query.push_str(" AND session_id = $session_id");
                query_builder = query_builder.bind(("session_id", session_id.to_string()));
            }
        }

        query.push_str(" ORDER BY room_id, session_id");

        let mut result = query_builder.await?;
        let keys_data: Vec<Value> = result.take(0)?;

        let mut rooms: HashMap<String, RoomKeys> = HashMap::new();

        for key_data in keys_data {
            if let (
                Some(room_id),
                Some(session_id),
                Some(first_message_index),
                Some(forwarded_count),
                Some(is_verified),
                Some(session_data),
            ) = (
                key_data.get("room_id").and_then(|v| v.as_str()),
                key_data.get("session_id").and_then(|v| v.as_str()),
                key_data.get("first_message_index").and_then(|v| v.as_u64()),
                key_data.get("forwarded_count").and_then(|v| v.as_u64()),
                key_data.get("is_verified").and_then(|v| v.as_bool()),
                key_data.get("session_data"),
            ) {
                let room_keys = rooms
                    .entry(room_id.to_string())
                    .or_insert_with(|| RoomKeys { sessions: HashMap::new() });

                room_keys.sessions.insert(session_id.to_string(), RoomKeyBackupData {
                    first_message_index: first_message_index as u32,
                    forwarded_count: forwarded_count as u32,
                    is_verified,
                    session_data: session_data.clone(),
                });
            }
        }

        Ok(RoomKeysResponse { rooms })
    }

    /// Delete room keys from backup (optionally filtered by room/session)
    pub async fn delete_room_keys(
        &self,
        user_id: &str,
        version: &str,
        room_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<(), RepositoryError> {
        // Verify the version exists and belongs to the user
        if !self.validate_backup_access(user_id, version).await? {
            return Err(RepositoryError::NotFound {
                entity_type: "Backup version".to_string(),
                id: format!("{}:{}", user_id, version),
            });
        }

        let mut query = "
            DELETE FROM room_key_backups
            WHERE user_id = $user_id AND version = $version
        "
        .to_string();

        let mut query_builder = self
            .db
            .query(&query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()));

        // Add room filter if specified
        if let Some(room_id) = room_id {
            query.push_str(" AND room_id = $room_id");
            query_builder = query_builder.bind(("room_id", room_id.to_string()));

            // Add session filter if both room and session specified
            if let Some(session_id) = session_id {
                query.push_str(" AND session_id = $session_id");
                query_builder = query_builder.bind(("session_id", session_id.to_string()));
            }
        }

        query_builder.await?;

        // Update the backup version count and etag
        self.update_backup_count_and_etag(user_id, version).await?;

        Ok(())
    }

    /// Validate that a user has access to a backup version
    pub async fn validate_backup_access(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT version FROM room_key_backup_versions
            WHERE user_id = $user_id AND version = $version
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        let versions: Vec<Value> = result.take(0)?;
        Ok(!versions.is_empty())
    }

    /// Get the next version number for a user
    async fn get_next_version_number(&self, user_id: &str) -> Result<u32, RepositoryError> {
        let query = "
            SELECT version FROM room_key_backup_versions
            WHERE user_id = $user_id
            ORDER BY version DESC
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let versions: Vec<Value> = result.take(0)?;

        if let Some(version_data) = versions.first()
            && let Some(version_str) = version_data.get("version").and_then(|v| v.as_str())
            && let Ok(version_num) = version_str.parse::<u32>() {
            return Ok(version_num + 1);
        }

        // Start with version 1 if no versions exist
        Ok(1)
    }

    /// Update backup count and etag after key changes
    async fn update_backup_count_and_etag(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<(), RepositoryError> {
        // Count current keys
        let count_query = "
            SELECT COUNT(*) as count FROM room_key_backups
            WHERE user_id = $user_id AND version = $version
            GROUP ALL
        ";

        let mut result = self
            .db
            .query(count_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        let counts: Vec<Value> = result.take(0)?;
        let count = counts
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Update version with new count and etag
        let new_etag = format!("etag_{}", Uuid::new_v4());
        let update_query = "
            UPDATE room_key_backup_versions SET
                count = $count,
                etag = $etag,
                updated_at = time::now()
            WHERE user_id = $user_id AND version = $version
        ";

        self.db
            .query(update_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .bind(("count", count))
            .bind(("etag", new_etag))
            .await?;

        Ok(())
    }

    /// Convert database value to RoomKeyBackupVersion
    fn value_to_backup_version(
        &self,
        value: Value,
    ) -> Result<Option<RoomKeyBackupVersion>, RepositoryError> {
        if let (Some(version), Some(algorithm), Some(auth_data), Some(count), Some(etag)) = (
            value.get("version").and_then(|v| v.as_str()),
            value.get("algorithm").and_then(|v| v.as_str()),
            value.get("auth_data"),
            value.get("count").and_then(|v| v.as_u64()),
            value.get("etag").and_then(|v| v.as_str()),
        ) {
            Ok(Some(RoomKeyBackupVersion {
                version: version.to_string(),
                algorithm: algorithm.to_string(),
                auth_data: auth_data.clone(),
                count,
                etag: etag.to_string(),
            }))
        } else {
            Ok(None)
        }
    }
}
