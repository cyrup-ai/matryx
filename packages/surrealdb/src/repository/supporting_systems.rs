use crate::repository::error::RepositoryError;
use crate::repository::{
    crypto_keys::{CryptoKeysRepository, DeviceKeysQuery, DeviceKeysResponse, OneTimeKeysClaim, OneTimeKeysResponse, CrossSigningKey},
    event::EventRepository,
    reactions::ReactionsRepository,
    room_keys::{RoomKeysRepository, RoomKeyBackupData, RoomKeysResponse},
    search::{SearchRepository, SearchCriteria, SearchResults},
    server_notices::ServerNoticesRepository,
    to_device::ToDeviceRepository,
};
use matryx_entity::types::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Connection, Surreal, engine::any::Any};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactionAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyUpload {
    pub device_keys: Option<HashMap<String, Value>>,
    pub one_time_keys: Option<HashMap<String, Value>>,
    pub fallback_keys: Option<HashMap<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyUploadResponse {
    pub one_time_key_counts: HashMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoomKeyBackupOperation {
    CreateVersion { algorithm: String, auth_data: Value },
    GetVersion { version: Option<String> },
    UpdateVersion { version: String, algorithm: String, auth_data: Value },
    DeleteVersion { version: String },
    StoreKeys { version: String, room_id: String, session_id: String, key_data: RoomKeyBackupData },
    GetKeys { version: String, room_id: Option<String>, session_id: Option<String> },
    DeleteKeys { version: String, room_id: Option<String>, session_id: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoomKeyBackupResponse {
    Version(String),
    BackupVersion(crate::repository::room_keys::RoomKeyBackupVersion),
    Keys(RoomKeysResponse),
    Success,
}

pub struct SupportingSystemsService {
    db: Surreal<Any>,
    reactions_repo: ReactionsRepository<Any>,
    server_notices_repo: ServerNoticesRepository<Any>,
    crypto_keys_repo: CryptoKeysRepository<Any>,
    search_repo: SearchRepository,
    to_device_repo: ToDeviceRepository,
    room_keys_repo: RoomKeysRepository<Any>,
    event_repo: EventRepository<Any>,
}

impl SupportingSystemsService {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            db: db.clone(),
            reactions_repo: ReactionsRepository::new(db.clone()),
            server_notices_repo: ServerNoticesRepository::new(db.clone()),
            crypto_keys_repo: CryptoKeysRepository::new(db.clone()),
            search_repo: SearchRepository::new(db.clone()),
            to_device_repo: ToDeviceRepository::new(db.clone()),
            room_keys_repo: RoomKeysRepository::new(db.clone()),
            event_repo: EventRepository::new(db),
        }
    }

    /// Handle reaction addition or removal
    pub async fn handle_reaction(
        &self,
        room_id: &str,
        event_id: &str,
        user_id: &str,
        reaction_key: &str,
        action: ReactionAction,
    ) -> Result<(), RepositoryError> {
        // Validate reaction permissions
        if !self.reactions_repo.validate_reaction_permissions(room_id, user_id, event_id).await? {
            return Err(RepositoryError::Forbidden {
                reason: format!("User {} cannot react to event {} in room {}", user_id, event_id, room_id),
            });
        }

        match action {
            ReactionAction::Add => {
                self.reactions_repo.add_reaction(room_id, event_id, user_id, reaction_key).await?;
            }
            ReactionAction::Remove => {
                self.reactions_repo.remove_reaction(room_id, event_id, user_id, reaction_key).await?;
            }
        }

        Ok(())
    }

    /// Handle event replacement/editing
    pub async fn handle_event_replacement(
        &self,
        room_id: &str,
        original_event_id: &str,
        replacement_event: &Event,
    ) -> Result<(), RepositoryError> {
        // Validate that the user can replace this event
        let original_event = self.event_repo.get_by_id(original_event_id).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Original event".to_string(),
                id: original_event_id.to_string(),
            })?;

        // Only the original sender can replace an event
        if original_event.sender != replacement_event.sender {
            return Err(RepositoryError::Forbidden {
                reason: format!("User {} cannot replace event {} in room {}", replacement_event.sender, original_event_id, room_id),
            });
        }

        self.event_repo.create_replacement_event(room_id, original_event_id, replacement_event).await?;

        Ok(())
    }

    /// Send a server notice to a user
    pub async fn send_server_notice(
        &self,
        user_id: &str,
        notice_type: &str,
        content: Value,
    ) -> Result<(), RepositoryError> {
        let _notice_id = self.server_notices_repo.send_server_notice(user_id, notice_type, content).await?;
        Ok(())
    }

    /// Upload cryptographic keys for a user/device
    pub async fn upload_crypto_keys(
        &self,
        user_id: &str,
        device_id: &str,
        keys: &KeyUpload,
    ) -> Result<KeyUploadResponse, RepositoryError> {
        // Upload device keys if provided
        if let Some(device_keys_data) = &keys.device_keys {
            // Convert to DeviceKeys structure
            let device_keys = crate::repository::crypto_keys::DeviceKeys {
                user_id: user_id.to_string(),
                device_id: device_id.to_string(),
                algorithms: device_keys_data.get("algorithms")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default(),
                keys: device_keys_data.get("keys")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default(),
                signatures: device_keys_data.get("signatures")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default(),
            };

            self.crypto_keys_repo.upload_device_keys(user_id, device_id, &device_keys).await?;
        }

        // Upload one-time keys if provided
        if let Some(one_time_keys) = &keys.one_time_keys {
            self.crypto_keys_repo.upload_one_time_keys(user_id, device_id, one_time_keys).await?;
        }

        // TODO: Handle fallback keys if needed

        // Count remaining one-time keys
        let one_time_key_counts = self.get_one_time_key_counts(user_id, device_id).await?;

        Ok(KeyUploadResponse {
            one_time_key_counts,
        })
    }

    /// Query cryptographic keys for users/devices
    pub async fn query_crypto_keys(
        &self,
        query: &DeviceKeysQuery,
    ) -> Result<DeviceKeysResponse, RepositoryError> {
        self.crypto_keys_repo.query_device_keys(query).await
    }

    /// Claim one-time keys for encryption
    pub async fn claim_one_time_keys(
        &self,
        claim: &OneTimeKeysClaim,
    ) -> Result<OneTimeKeysResponse, RepositoryError> {
        self.crypto_keys_repo.claim_one_time_keys(claim).await
    }

    /// Search events for a user
    pub async fn search_events(
        &self,
        user_id: &str,
        criteria: &SearchCriteria,
    ) -> Result<SearchResults, RepositoryError> {
        self.search_repo.search_events(user_id, criteria).await
    }

    /// Search user directory
    pub async fn search_user_directory(
        &self,
        search_term: &str,
        limit: Option<u32>,
    ) -> Result<matryx_entity::types::UserDirectoryResponse, RepositoryError> {
        self.search_repo.get_user_directory(search_term, limit).await
    }

    /// Send to-device messages
    pub async fn send_to_device_messages(
        &self,
        sender_id: &str,
        event_type: &str,
        messages: &HashMap<String, HashMap<String, Value>>,
    ) -> Result<(), RepositoryError> {
        // Validate permissions for each recipient
        for recipient_id in messages.keys() {
            if !self.to_device_repo.validate_to_device_permissions(sender_id, recipient_id).await? {
                return Err(RepositoryError::Forbidden {
                    reason: format!("User {} cannot send to-device message to user {}", sender_id, recipient_id),
                });
            }
        }

        self.to_device_repo.send_to_device(sender_id, event_type, messages).await
    }

    /// Manage room key backup operations
    pub async fn manage_room_key_backup(
        &self,
        user_id: &str,
        operation: RoomKeyBackupOperation,
    ) -> Result<RoomKeyBackupResponse, RepositoryError> {
        match operation {
            RoomKeyBackupOperation::CreateVersion { algorithm, auth_data } => {
                let version = self.room_keys_repo.create_backup_version(user_id, &algorithm, auth_data).await?;
                Ok(RoomKeyBackupResponse::Version(version))
            }
            RoomKeyBackupOperation::GetVersion { version } => {
                let backup_version = self.room_keys_repo.get_backup_version(user_id, version.as_deref()).await?
                    .ok_or_else(|| RepositoryError::NotFound {
                        entity_type: "Backup version".to_string(),
                        id: version.unwrap_or_else(|| "latest".to_string()),
                    })?;
                Ok(RoomKeyBackupResponse::BackupVersion(backup_version))
            }
            RoomKeyBackupOperation::UpdateVersion { version, algorithm, auth_data } => {
                self.room_keys_repo.update_backup_version(user_id, &version, &algorithm, auth_data).await?;
                Ok(RoomKeyBackupResponse::Success)
            }
            RoomKeyBackupOperation::DeleteVersion { version } => {
                self.room_keys_repo.delete_backup_version(user_id, &version).await?;
                Ok(RoomKeyBackupResponse::Success)
            }
            RoomKeyBackupOperation::StoreKeys { version, room_id, session_id, key_data } => {
                self.room_keys_repo.store_room_keys(user_id, &version, &room_id, &session_id, &key_data).await?;
                Ok(RoomKeyBackupResponse::Success)
            }
            RoomKeyBackupOperation::GetKeys { version, room_id, session_id } => {
                let keys = self.room_keys_repo.get_room_keys(
                    user_id,
                    &version,
                    room_id.as_deref(),
                    session_id.as_deref(),
                ).await?;
                Ok(RoomKeyBackupResponse::Keys(keys))
            }
            RoomKeyBackupOperation::DeleteKeys { version, room_id, session_id } => {
                self.room_keys_repo.delete_room_keys(
                    user_id,
                    &version,
                    room_id.as_deref(),
                    session_id.as_deref(),
                ).await?;
                Ok(RoomKeyBackupResponse::Success)
            }
        }
    }

    /// Upload cross-signing keys
    pub async fn upload_cross_signing_keys(
        &self,
        user_id: &str,
        master_key: Option<&CrossSigningKey>,
        self_signing_key: Option<&CrossSigningKey>,
        user_signing_key: Option<&CrossSigningKey>,
    ) -> Result<(), RepositoryError> {
        self.crypto_keys_repo.upload_signing_keys(
            user_id,
            master_key,
            self_signing_key,
            user_signing_key,
        ).await
    }

    /// Upload signatures for cross-signing verification
    pub async fn upload_signatures(
        &self,
        user_id: &str,
        keys: &Value,
        signatures: &HashMap<String, HashMap<String, Value>>,
    ) -> Result<(), RepositoryError> {
        // Extract signatures from the nested HashMap for validation
        let flattened_signatures: HashMap<String, Value> = signatures
            .iter()
            .flat_map(|(_, sig_map)| sig_map.clone())
            .collect();

        // Validate signatures per Matrix spec (verify against canonical JSON)
        if !self.crypto_keys_repo.validate_key_signatures(user_id, keys, &flattened_signatures).await? {
            return Err(RepositoryError::Validation {
                field: "signatures".to_string(),
                message: "Invalid key signatures".to_string(),
            });
        }

        self.crypto_keys_repo.upload_signatures(user_id, signatures).await
    }

    /// Index an event for search
    pub async fn index_event_for_search(
        &self,
        room_id: &str,
        event: &Event,
    ) -> Result<(), RepositoryError> {
        // Convert Event to Value for search indexing
        let event_value = serde_json::to_value(event)
            .map_err(|e| RepositoryError::SerializationError { message: format!("Failed to serialize event: {}", e) })?;
        self.search_repo.index_event_for_search(&event_value, room_id).await
    }

    /// Remove an event from search index
    pub async fn remove_event_from_search(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<(), RepositoryError> {
        self.search_repo.remove_event_from_search(event_id).await
    }

    /// Clean up old delivered to-device messages
    pub async fn cleanup_delivered_to_device_messages(
        &self,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, RepositoryError> {
        self.to_device_repo.cleanup_delivered_messages(cutoff).await
    }

    /// Clean up old server notices
    pub async fn cleanup_old_server_notices(
        &self,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, RepositoryError> {
        self.server_notices_repo.cleanup_old_notices(cutoff).await
    }

    /// Clean up reactions for a redacted event
    pub async fn cleanup_reactions_for_redacted_event(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<(), RepositoryError> {
        self.reactions_repo.cleanup_reactions_for_redacted_event(room_id, event_id).await
    }

    /// Get one-time key counts for a device
    async fn get_one_time_key_counts(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<HashMap<String, u32>, RepositoryError> {
        self.crypto_keys_repo.get_one_time_key_counts(user_id, device_id).await
    }

    /// Get system statistics and counts
    pub async fn get_system_counts(&self) -> Result<SystemCounts, RepositoryError> {
        // Count total users
        let users_query = "SELECT count() as count FROM users";
        let mut users_result = self.db.query(users_query).await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_users".to_string(),
            })?;
        let users_count_data: Vec<serde_json::Value> = users_result.take(0)
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_users_parse".to_string(),
            })?;
        let total_users = users_count_data
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Count total rooms by type
        let rooms_query = "SELECT room_type, count() as count FROM room GROUP BY room_type";
        let mut rooms_result = self.db.query(rooms_query).await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_rooms".to_string(),
            })?;
        let rooms_data: Vec<serde_json::Value> = rooms_result.take(0)
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_rooms_parse".to_string(),
            })?;

        let mut room_counts = HashMap::new();
        let mut total_rooms = 0u64;
        for room_data in rooms_data {
            if let (Some(room_type), Some(count)) = (
                room_data.get("room_type").and_then(|v| v.as_str()),
                room_data.get("count").and_then(|v| v.as_u64()),
            ) {
                room_counts.insert(room_type.to_string(), count);
                total_rooms += count;
            }
        }

        // Count total events
        let events_query = "SELECT count() as count FROM event";
        let mut events_result = self.db.query(events_query).await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_events".to_string(),
            })?;
        let events_count_data: Vec<serde_json::Value> = events_result.take(0)
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_events_parse".to_string(),
            })?;
        let total_events = events_count_data
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Count federation peers (distinct servers from events)
        let peers_query = "SELECT count(DISTINCT server_name) as count FROM (
            SELECT string::split(sender, ':')[1] as server_name FROM event WHERE sender LIKE '%:%'
        )";
        let mut peers_result = self.db.query(peers_query).await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_federation_peers".to_string(),
            })?;
        let peers_count_data: Vec<serde_json::Value> = peers_result.take(0)
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "count_federation_peers_parse".to_string(),
            })?;
        let federation_peers = peers_count_data
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(SystemCounts {
            total_users,
            total_rooms,
            room_counts,
            total_events,
            federation_peers,
        })
    }
}

/// System statistics and counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCounts {
    pub total_users: u64,
    pub total_rooms: u64,
    pub room_counts: HashMap<String, u64>,
    pub total_events: u64,
    pub federation_peers: u64,
}