use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVersion {
    pub version: String,
    pub algorithm: String,
    pub auth_data: serde_json::Value,
    pub count: u64,
    pub etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedRoomKey {
    pub room_id: String,
    pub session_id: String,
    pub first_message_index: u32,
    pub forwarded_count: u32,
    pub is_verified: bool,
    pub session_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStatistics {
    pub total_keys: u64,
    pub total_rooms: u64,
    pub last_backup: Option<DateTime<Utc>>,
    pub backup_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVersionRecord {
    pub id: String,
    pub user_id: String,
    pub version: String,
    pub algorithm: String,
    pub auth_data: serde_json::Value,
    pub count: u64,
    pub etag: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeyBackupRecord {
    pub id: String,
    pub user_id: String,
    pub version: String,
    pub room_id: String,
    pub session_id: String,
    pub key_data: EncryptedRoomKey,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct KeyBackupRepository {
    db: Surreal<Any>,
}

impl KeyBackupRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create_backup_version(
        &self,
        user_id: &str,
        algorithm: &str,
        auth_data: &serde_json::Value,
    ) -> Result<String, RepositoryError> {
        // Generate a new version ID
        let version = format!("v{}", Utc::now().timestamp());
        let etag = format!("etag_{}", uuid::Uuid::new_v4());

        let record = BackupVersionRecord {
            id: format!("backup_version:{}:{}", user_id, version),
            user_id: user_id.to_string(),
            version: version.clone(),
            algorithm: algorithm.to_string(),
            auth_data: auth_data.clone(),
            count: 0,
            etag,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<BackupVersionRecord> = self
            .db
            .create(("backup_version", format!("{}:{}", user_id, version)))
            .content(record)
            .await?;

        Ok(version)
    }

    pub async fn get_backup_version(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<Option<BackupVersion>, RepositoryError> {
        let record: Option<BackupVersionRecord> = self
            .db
            .select(("backup_version", format!("{}:{}", user_id, version)))
            .await?;

        Ok(record.map(|r| {
            BackupVersion {
                version: r.version,
                algorithm: r.algorithm,
                auth_data: r.auth_data,
                count: r.count,
                etag: r.etag,
            }
        }))
    }

    pub async fn get_latest_backup_version(
        &self,
        user_id: &str,
    ) -> Result<Option<BackupVersion>, RepositoryError> {
        let query = "SELECT * FROM backup_version WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let records: Vec<BackupVersionRecord> = result.take(0)?;

        Ok(records.into_iter().next().map(|r| {
            BackupVersion {
                version: r.version,
                algorithm: r.algorithm,
                auth_data: r.auth_data,
                count: r.count,
                etag: r.etag,
            }
        }))
    }

    pub async fn update_backup_version(
        &self,
        user_id: &str,
        version: &str,
        auth_data: &serde_json::Value,
    ) -> Result<(), RepositoryError> {
        let record_id = format!("{}:{}", user_id, version);

        // Get existing record
        let existing: Option<BackupVersionRecord> =
            self.db.select(("backup_version", &record_id)).await?;

        if let Some(mut record) = existing {
            record.auth_data = auth_data.clone();
            record.updated_at = Utc::now();
            record.etag = format!("etag_{}", uuid::Uuid::new_v4());

            let _: Option<BackupVersionRecord> =
                self.db.update(("backup_version", record_id)).content(record).await?;

            Ok(())
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "backup_version".to_string(),
                id: version.to_string(),
            })
        }
    }

    pub async fn delete_backup_version(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<(), RepositoryError> {
        // Delete all room keys for this backup version
        let room_keys_query =
            "DELETE FROM room_key_backup WHERE user_id = $user_id AND version = $version";
        self.db
            .query(room_keys_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        // Delete the backup version record
        let _: Option<BackupVersionRecord> = self
            .db
            .delete(("backup_version", format!("{}:{}", user_id, version)))
            .await?;

        Ok(())
    }

    pub async fn store_room_key(
        &self,
        user_id: &str,
        version: &str,
        room_id: &str,
        session_id: &str,
        key_data: &EncryptedRoomKey,
    ) -> Result<(), RepositoryError> {
        let record = RoomKeyBackupRecord {
            id: format!("room_key_backup:{}:{}:{}:{}", user_id, version, room_id, session_id),
            user_id: user_id.to_string(),
            version: version.to_string(),
            room_id: room_id.to_string(),
            session_id: session_id.to_string(),
            key_data: key_data.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<RoomKeyBackupRecord> = self
            .db
            .create((
                "room_key_backup",
                format!("{}:{}:{}:{}", user_id, version, room_id, session_id),
            ))
            .content(record)
            .await?;

        // Update the count in the backup version
        self.increment_backup_count(user_id, version).await?;

        Ok(())
    }

    pub async fn get_room_key(
        &self,
        user_id: &str,
        version: &str,
        room_id: &str,
        session_id: &str,
    ) -> Result<Option<EncryptedRoomKey>, RepositoryError> {
        let record: Option<RoomKeyBackupRecord> = self
            .db
            .select((
                "room_key_backup",
                format!("{}:{}:{}:{}", user_id, version, room_id, session_id),
            ))
            .await?;

        Ok(record.map(|r| r.key_data))
    }

    pub async fn get_room_keys(
        &self,
        user_id: &str,
        version: &str,
        room_id: Option<&str>,
    ) -> Result<Vec<EncryptedRoomKey>, RepositoryError> {
        let (query, room_filter) = if let Some(room_id) = room_id {
            (
                "SELECT * FROM room_key_backup WHERE user_id = $user_id AND version = $version AND room_id = $room_id",
                Some(room_id.to_string()),
            )
        } else {
            ("SELECT * FROM room_key_backup WHERE user_id = $user_id AND version = $version", None)
        };

        let mut query_builder = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()));

        if let Some(room_id) = room_filter {
            query_builder = query_builder.bind(("room_id", room_id));
        }

        let mut result = query_builder.await?;
        let records: Vec<RoomKeyBackupRecord> = result.take(0)?;

        Ok(records.into_iter().map(|r| r.key_data).collect())
    }

    pub async fn delete_room_key(
        &self,
        user_id: &str,
        version: &str,
        room_id: &str,
        session_id: &str,
    ) -> Result<(), RepositoryError> {
        let _: Option<RoomKeyBackupRecord> = self
            .db
            .delete((
                "room_key_backup",
                format!("{}:{}:{}:{}", user_id, version, room_id, session_id),
            ))
            .await?;

        // Decrement the count in the backup version
        self.decrement_backup_count(user_id, version).await?;

        Ok(())
    }

    pub async fn get_backup_statistics(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<BackupStatistics, RepositoryError> {
        // Get total number of keys
        let count_query = "SELECT count() AS total FROM room_key_backup WHERE user_id = $user_id AND version = $version";
        let mut count_result = self
            .db
            .query(count_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct CountResult {
            total: u64,
        }

        let count_records: Vec<CountResult> = count_result.take(0)?;
        let total_keys = count_records.into_iter().next().map(|r| r.total).unwrap_or(0);

        // Get total number of rooms
        let rooms_query = "SELECT DISTINCT room_id FROM room_key_backup WHERE user_id = $user_id AND version = $version";
        let mut rooms_result = self
            .db
            .query(rooms_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct RoomResult {
            _room_id: String,
        }

        let room_records: Vec<RoomResult> = rooms_result.take(0)?;
        let total_rooms = room_records.len() as u64;

        // Get last backup time
        let last_backup_query = "SELECT updated_at FROM room_key_backup WHERE user_id = $user_id AND version = $version ORDER BY updated_at DESC LIMIT 1";
        let mut last_result = self
            .db
            .query(last_backup_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct LastBackupResult {
            updated_at: DateTime<Utc>,
        }

        let last_backup_records: Vec<LastBackupResult> = last_result.take(0)?;
        let last_backup = last_backup_records.into_iter().next().map(|r| r.updated_at);

        // Estimate backup size (rough calculation based on JSON serialization)
        let backup_size_bytes = total_keys * 1024; // Rough estimate of 1KB per key

        Ok(BackupStatistics {
            total_keys,
            total_rooms,
            last_backup,
            backup_size_bytes,
        })
    }

    async fn increment_backup_count(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<(), RepositoryError> {
        let update_query = "UPDATE backup_version SET count = count + 1, updated_at = $now, etag = $etag WHERE user_id = $user_id AND version = $version";
        self.db
            .query(update_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .bind(("now", Utc::now()))
            .bind(("etag", format!("etag_{}", uuid::Uuid::new_v4())))
            .await?;

        Ok(())
    }

    async fn decrement_backup_count(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<(), RepositoryError> {
        let update_query = "UPDATE backup_version SET count = math::max(count - 1, 0), updated_at = $now, etag = $etag WHERE user_id = $user_id AND version = $version";
        self.db
            .query(update_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .bind(("now", Utc::now()))
            .bind(("etag", format!("etag_{}", uuid::Uuid::new_v4())))
            .await?;

        Ok(())
    }

    pub async fn get_all_backup_versions(
        &self,
        user_id: &str,
    ) -> Result<Vec<BackupVersion>, RepositoryError> {
        let query =
            "SELECT * FROM backup_version WHERE user_id = $user_id ORDER BY created_at DESC";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let records: Vec<BackupVersionRecord> = result.take(0)?;

        Ok(records
            .into_iter()
            .map(|r| {
                BackupVersion {
                    version: r.version,
                    algorithm: r.algorithm,
                    auth_data: r.auth_data,
                    count: r.count,
                    etag: r.etag,
                }
            })
            .collect())
    }

    pub async fn delete_all_backup_data(&self, user_id: &str) -> Result<(), RepositoryError> {
        // Delete all room key backups for the user
        let room_keys_query = "DELETE FROM room_key_backup WHERE user_id = $user_id";
        self.db
            .query(room_keys_query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        // Delete all backup versions for the user
        let versions_query = "DELETE FROM backup_version WHERE user_id = $user_id";
        self.db
            .query(versions_query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        Ok(())
    }

    pub async fn backup_room_keys_batch(
        &self,
        user_id: &str,
        version: &str,
        room_keys: &[(String, String, EncryptedRoomKey)],
    ) -> Result<u64, RepositoryError> {
        let mut successful_count = 0u64;

        for (room_id, session_id, key_data) in room_keys {
            match self.store_room_key(user_id, version, room_id, session_id, key_data).await {
                Ok(_) => successful_count += 1,
                Err(e) => {
                    // Log error but continue with other keys
                    tracing::warn!(
                        "Failed to backup room key for {}/{}: {}",
                        room_id,
                        session_id,
                        e
                    );
                },
            }
        }

        Ok(successful_count)
    }

    pub async fn verify_backup_integrity(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<bool, RepositoryError> {
        // Get backup version record
        let backup_version = self.get_backup_version(user_id, version).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "backup_version".to_string(),
                id: version.to_string(),
            }
        })?;

        // Count actual room keys
        let count_query = "SELECT count() AS actual_count FROM room_key_backup WHERE user_id = $user_id AND version = $version";
        let mut result = self
            .db
            .query(count_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("version", version.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct CountResult {
            actual_count: u64,
        }

        let counts: Vec<CountResult> = result.take(0)?;
        let actual_count = counts.into_iter().next().map(|r| r.actual_count).unwrap_or(0);

        Ok(backup_version.count == actual_count)
    }
}
