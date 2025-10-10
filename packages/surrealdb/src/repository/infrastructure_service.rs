use crate::repository::{
    auth::AuthRepository,
    device::{ClientDeviceInfo, DeviceRepository},
    directory::{DirectoryRepository, PublicRoomsResponse},
    error::RepositoryError,
    key_server::{KeyServerRepository, ServerKeys},
    registration::{RegistrationRepository, RegistrationResult},
    transaction::TransactionRepository,
    websocket::WebSocketRepository,
};
use chrono::Utc;
use tracing;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub next_batch: String,
    pub rooms: SyncRooms,
    pub presence: Option<Value>,
    pub account_data: Option<Value>,
    pub to_device: Option<Value>,
    pub device_lists: Option<Value>,
    pub device_one_time_keys_count: Option<HashMap<String, i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRooms {
    pub join: HashMap<String, SyncJoinedRoom>,
    pub invite: HashMap<String, SyncInvitedRoom>,
    pub leave: HashMap<String, SyncLeftRoom>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncJoinedRoom {
    pub state: Option<Value>,
    pub timeline: Option<Value>,
    pub ephemeral: Option<Value>,
    pub account_data: Option<Value>,
    pub unread_notifications: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncInvitedRoom {
    pub invite_state: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLeftRoom {
    pub state: Option<Value>,
    pub timeline: Option<Value>,
    pub account_data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeysResponse {
    pub server_keys: Vec<ServerKeys>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_ms: Option<i64>,
    pub device_id: String,
    pub well_known: Option<Value>,
    pub home_server: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicesResponse {
    pub devices: Vec<ClientDeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AliasOperation {
    Create { room_id: String },
    Get,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasResponse {
    pub room_id: Option<String>,
    pub servers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinedRoomsResponse {
    pub joined_rooms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoisResponse {
    pub user_id: String,
    pub devices: HashMap<String, DeviceWhoisInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceWhoisInfo {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub connections: Vec<ConnectionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub ip: Option<String>,
    pub last_seen: Option<i64>,
    pub user_agent: Option<String>,
}

pub struct InfrastructureService<C: Connection> {
    websocket_repo: WebSocketRepository<C>,
    transaction_repo: TransactionRepository<C>,
    key_server_repo: KeyServerRepository<C>,
    registration_repo: RegistrationRepository<C>,
    directory_repo: DirectoryRepository<C>,
    device_repo: DeviceRepository,
    auth_repo: AuthRepository<C>,
}

impl<C: Connection> InfrastructureService<C> {
    pub fn new(
        websocket_repo: WebSocketRepository<C>,
        transaction_repo: TransactionRepository<C>,
        key_server_repo: KeyServerRepository<C>,
        registration_repo: RegistrationRepository<C>,
        directory_repo: DirectoryRepository<C>,
        device_repo: DeviceRepository,
        auth_repo: AuthRepository<C>,
    ) -> Self {
        Self {
            websocket_repo,
            transaction_repo,
            key_server_repo,
            registration_repo,
            directory_repo,
            device_repo,
            auth_repo,
        }
    }

    /// Handle WebSocket sync request with efficient data gathering
    pub async fn handle_websocket_sync(
        &self,
        user_id: &str,
        device_id: &str,
        since: Option<&str>,
    ) -> Result<SyncResponse, RepositoryError> {
        // Implement proper incremental sync based on since parameter
        let is_initial_sync = since.is_none();
        let sync_timestamp = if let Some(since_token) = since {
            tracing::debug!("Processing incremental sync for user {} device {} since {}", user_id, device_id, since_token);
            // Parse timestamp from since token (format: s{timestamp})
            since_token.strip_prefix('s')
                .and_then(|ts| ts.parse::<i64>().ok())
                .unwrap_or(0)
        } else {
            tracing::debug!("Processing initial sync for user {} device {}", user_id, device_id);
            0 // Initial sync gets all data
        };

        // Get user's room memberships
        let memberships = self.websocket_repo.get_user_memberships_for_sync(user_id).await?;

        // Build sync response based on memberships
        let mut joined_rooms = HashMap::new();
        let mut invited_rooms = HashMap::new();
        let mut left_rooms = HashMap::new();

        for membership in memberships {
            match membership.membership_state.as_str() {
                "join" => {
                    joined_rooms.insert(membership.room_id, SyncJoinedRoom {
                        state: None,
                        timeline: None,
                        ephemeral: None,
                        account_data: None,
                        unread_notifications: None,
                    });
                },
                "invite" => {
                    invited_rooms
                        .insert(membership.room_id, SyncInvitedRoom { invite_state: None });
                },
                "leave" => {
                    left_rooms.insert(membership.room_id, SyncLeftRoom {
                        state: None,
                        timeline: None,
                        account_data: None,
                    });
                },
                _ => {},
            }
        }

        let next_batch = format!("s{}", Utc::now().timestamp_millis());

        // Implement device-specific features for E2EE support
        // Fetch to-device events for this specific device since last sync
        let to_device_events = if is_initial_sync {
            // For initial sync, get all pending to-device events
            self.device_repo.get_pending_to_device_events(user_id, device_id).await
                .map_err(|_| RepositoryError::Database(surrealdb::Error::msg("Failed to fetch to-device events")))?
        } else {
            // For incremental sync, get events since sync_timestamp
            self.device_repo.get_to_device_events_since(user_id, device_id, sync_timestamp).await
                .map_err(|_| RepositoryError::Database(surrealdb::Error::msg("Failed to fetch incremental to-device events")))?
        };

        // Check for device list updates since the last sync
        let device_lists = if !is_initial_sync {
            Some(self.device_repo.get_device_list_changes_since(user_id, sync_timestamp).await
                .map_err(|_| RepositoryError::Database(surrealdb::Error::msg("Failed to fetch device list changes")))?)
        } else {
            None // Initial sync doesn't need device list changes
        };

        // Get one-time key counts for this device
        let device_one_time_keys_count = self.device_repo.get_one_time_key_counts(user_id, device_id).await
            .map_err(|_| RepositoryError::Database(surrealdb::Error::msg("Failed to fetch OTK counts")))?;
        
        Ok(SyncResponse {
            next_batch,
            rooms: SyncRooms {
                join: joined_rooms,
                invite: invited_rooms,
                leave: left_rooms,
            },
            presence: None,
            account_data: None,
            to_device: Some(to_device_events),
            device_lists,
            device_one_time_keys_count: Some(device_one_time_keys_count),
        })
    }

    /// Register WebSocket connection for real-time updates
    pub async fn register_websocket_connection(
        &self,
        user_id: &str,
        device_id: &str,
        connection_id: &str,
    ) -> Result<(), RepositoryError> {
        self.websocket_repo
            .register_connection(user_id, device_id, connection_id)
            .await
    }

    /// Handle transaction deduplication for middleware
    pub async fn handle_transaction_deduplication(
        &self,
        user_id: &str,
        txn_id: &str,
        endpoint: &str,
    ) -> Result<Option<Value>, RepositoryError> {
        // Check if transaction already exists
        if self
            .transaction_repo
            .check_transaction_duplicate(user_id, txn_id, endpoint)
            .await?
        {
            // Return cached result
            self.transaction_repo
                .get_transaction_result(user_id, txn_id, endpoint)
                .await
        } else {
            // No duplicate found
            Ok(None)
        }
    }

    /// Store transaction result for deduplication
    pub async fn store_transaction_result(
        &self,
        user_id: &str,
        txn_id: &str,
        endpoint: &str,
        result: Value,
    ) -> Result<(), RepositoryError> {
        self.transaction_repo
            .store_transaction_result(user_id, txn_id, endpoint, result)
            .await
    }

    /// Get server keys for Matrix key server
    pub async fn get_server_keys(
        &self,
        server_name: &str,
        key_ids: Option<&[String]>,
    ) -> Result<ServerKeysResponse, RepositoryError> {
        let keys = self.key_server_repo.get_server_keys(server_name, key_ids).await?;
        Ok(ServerKeysResponse { server_keys: vec![keys] })
    }

    /// Store server signing key
    pub async fn store_signing_key(
        &self,
        server_name: &str,
        key_id: &str,
        key: &crate::repository::key_server::SigningKey,
    ) -> Result<(), RepositoryError> {
        self.key_server_repo.store_signing_key(server_name, key_id, key).await
    }

    /// Get signing key for server
    pub async fn get_signing_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<Option<crate::repository::key_server::SigningKey>, RepositoryError> {
        self.key_server_repo.get_signing_key(server_name, key_id).await
    }

    /// Verify key signature
    pub async fn verify_key_signature(
        &self,
        server_name: &str,
        key_id: &str,
        signature: &str,
        content: &[u8],
    ) -> Result<bool, RepositoryError> {
        self.key_server_repo
            .verify_key_signature(server_name, key_id, signature, content)
            .await
    }

    /// Get old verify keys for server key response
    pub async fn get_old_verify_keys(
        &self,
        server_name: &str,
    ) -> Result<HashMap<String, crate::repository::key_server::OldVerifyKey>, RepositoryError> {
        self.key_server_repo.get_old_verify_keys(server_name).await
    }

    /// Mark old keys as inactive during key rotation
    pub async fn mark_old_keys_inactive(&self, server_name: &str) -> Result<(), RepositoryError> {
        self.key_server_repo.mark_old_keys_inactive(server_name).await
    }

    /// Get server keys directly from repository (for caching logic)
    pub async fn get_server_keys_raw(
        &self,
        server_name: &str,
        key_ids: Option<&[String]>,
    ) -> Result<crate::repository::key_server::ServerKeys, RepositoryError> {
        self.key_server_repo.get_server_keys(server_name, key_ids).await
    }

    /// Store server keys in cache
    pub async fn store_server_keys(
        &self,
        server_name: &str,
        keys: &crate::repository::key_server::ServerKeys,
        valid_until: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        self.key_server_repo.store_server_keys(server_name, keys, valid_until).await
    }

    /// Register a new user with device
    pub async fn register_new_user(
        &self,
        username: &str,
        password: &str,
        device_id: Option<&str>,
        initial_device_display_name: Option<&str>,
    ) -> Result<RegistrationResult, RepositoryError> {
        self.register_new_user_with_options(username, password, device_id, initial_device_display_name, false).await
    }

    pub async fn register_new_user_with_options(
        &self,
        username: &str,
        password: &str,
        device_id: Option<&str>,
        initial_device_display_name: Option<&str>,
        enable_refresh_tokens: bool,
    ) -> Result<RegistrationResult, RepositoryError> {
        // Check username availability
        if !self.registration_repo.check_username_availability(username).await? {
            return Err(RepositoryError::Validation {
                field: "username".to_string(),
                message: "Username is already taken".to_string(),
            });
        }

        let user_id = format!("@{}:localhost", username);
        let default_device_id = format!("DEVICE_{}", uuid::Uuid::new_v4());
        let device_id = device_id.unwrap_or(&default_device_id);

        // Hash password (in real implementation, use proper password hashing)
        let password_hash = format!("hashed_{}", password);

        // Register user with refresh token option
        let mut result = self.registration_repo
            .register_user(&user_id, &password_hash, device_id, initial_device_display_name)
            .await?;

        // Generate and store refresh token if requested
        if enable_refresh_tokens {
            use crate::repository::auth::ExtendedRefreshToken;
            use chrono::Duration;
            
            let refresh_token = format!("syr_{}", uuid::Uuid::new_v4());
            let now = Utc::now();
            let expires_at = now + Duration::days(30);
            
            // Create extended refresh token record
            let refresh_record = ExtendedRefreshToken {
                token: refresh_token.clone(),
                user_id: user_id.clone(),
                device_id: device_id.to_string(),
                access_token: result.access_token.clone(),
                created_at: now,
                expires_at,
                used: false,
                revoked: false,
                rotation_count: 0,
                parent_token: None,
            };
            
            // Store in database
            self.auth_repo
                .store_extended_refresh_token(&refresh_record)
                .await
                .map_err(|e| {
                    RepositoryError::Database(
                        surrealdb::Error::msg(format!("Failed to store refresh token: {}", e))
                    )
                })?;
            
            result.refresh_token = Some(refresh_token);
            result.expires_in_ms = Some(Duration::hours(1).num_milliseconds());
        }

        Ok(result)
    }

    /// Login user with device
    pub async fn login_user(
        &self,
        user_id: &str,
        _password: &str,
        device_id: Option<&str>,
        _initial_device_display_name: Option<&str>,
    ) -> Result<LoginResponse, RepositoryError> {
        // In real implementation, verify password hash
        let default_device_id = format!("DEVICE_{}", uuid::Uuid::new_v4());
        let device_id = device_id.unwrap_or(&default_device_id);
        let access_token = format!("syt_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

        Ok(LoginResponse {
            user_id: user_id.to_string(),
            access_token,
            refresh_token: None,
            expires_in_ms: None,
            device_id: device_id.to_string(),
            well_known: None,
            home_server: "localhost".to_string(),
        })
    }

    /// Refresh access token
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<RefreshResponse, RepositoryError> {
        // In real implementation, validate refresh token
        let access_token = format!("syt_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

        Ok(RefreshResponse {
            access_token,
            refresh_token: Some(refresh_token.to_string()),
            expires_in_ms: None,
        })
    }

    /// Get user devices for device management
    pub async fn get_user_devices(
        &self,
        user_id: &str,
    ) -> Result<DevicesResponse, RepositoryError> {
        let devices = self.device_repo.get_user_devices_list(user_id).await?;
        Ok(DevicesResponse { devices })
    }

    /// Update device information
    pub async fn update_device(
        &self,
        user_id: &str,
        device_id: &str,
        display_name: Option<String>,
    ) -> Result<(), RepositoryError> {
        self.device_repo.update_device_info(user_id, device_id, display_name).await
    }

    /// Delete a device
    pub async fn delete_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        self.device_repo.delete_device(user_id, device_id).await
    }

    /// Get public rooms directory
    pub async fn get_public_rooms(
        &self,
        server: Option<&str>,
        limit: Option<u32>,
        since: Option<&str>,
    ) -> Result<PublicRoomsResponse, RepositoryError> {
        self.directory_repo.get_public_rooms(server, limit, since).await
    }

    /// Manage room aliases (create, get, delete)
    pub async fn manage_room_alias(
        &self,
        alias: &str,
        operation: AliasOperation,
        user_id: &str,
    ) -> Result<AliasResponse, RepositoryError> {
        match operation {
            AliasOperation::Create { room_id } => {
                self.directory_repo.create_room_alias(alias, &room_id, user_id).await?;
                Ok(AliasResponse {
                    room_id: Some(room_id),
                    servers: Some(vec!["localhost".to_string()]),
                })
            },
            AliasOperation::Get => {
                match self.directory_repo.get_room_alias(alias).await? {
                    Some(info) => {
                        Ok(AliasResponse {
                            room_id: Some(info.room_id),
                            servers: Some(info.servers),
                        })
                    },
                    None => {
                        Err(RepositoryError::NotFound {
                            entity_type: "Room alias".to_string(),
                            id: alias.to_string(),
                        })
                    },
                }
            },
            AliasOperation::Delete => {
                self.directory_repo.delete_room_alias(alias, user_id).await?;
                Ok(AliasResponse { room_id: None, servers: None })
            },
        }
    }

    /// Get joined rooms for a user
    pub async fn get_joined_rooms(
        &self,
        user_id: &str,
    ) -> Result<JoinedRoomsResponse, RepositoryError> {
        let memberships = self.websocket_repo.get_user_memberships_for_sync(user_id).await?;
        let joined_rooms: Vec<String> = memberships
            .into_iter()
            .filter(|m| m.membership_state == "join")
            .map(|m| m.room_id)
            .collect();

        Ok(JoinedRoomsResponse { joined_rooms })
    }

    /// Get user whois information for admin
    pub async fn get_user_whois_info(
        &self,
        user_id: &str,
        _requesting_user: &str,
    ) -> Result<WhoisResponse, RepositoryError> {
        // In real implementation, check admin permissions
        let devices = self.device_repo.get_user_devices_list(user_id).await?;
        let connections = self.websocket_repo.get_user_connections(user_id).await?;

        let mut device_info = HashMap::new();
        for device in devices {
            let device_connections: Vec<ConnectionInfo> = connections
                .iter()
                .filter(|c| c.device_id == device.device_id)
                .map(|c| {
                    ConnectionInfo {
                        ip: c.ip_address.clone(),
                        last_seen: Some(c.last_seen.timestamp()),
                        user_agent: c.user_agent.clone(),
                    }
                })
                .collect();

            device_info.insert(device.device_id, DeviceWhoisInfo {
                sessions: vec![SessionInfo { connections: device_connections }],
            });
        }

        Ok(WhoisResponse { user_id: user_id.to_string(), devices: device_info })
    }

    /// Clean up expired transactions using the internal transaction repository
    pub async fn cleanup_expired_transactions(
        &self,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, RepositoryError> {
        self.transaction_repo.cleanup_expired_transactions(cutoff).await
    }
}
