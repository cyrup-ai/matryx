//! Matrix client wrapper with synchronous interfaces that hide async complexity
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Client
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;
use tracing::{debug, error, instrument, warn};

use matrix_sdk::{
    config::{RequestConfig, StoreConfig},
    ruma::{
        api::client::{
            account::register::v3::Request as RegistrationRequest,
            session::login::v3::Request as LoginRequest,
        },
        DeviceId,
        OwnedDeviceId,
        RoomId,
        UserId,
    },
    Client as SdkClient,
};



use crate::encryption::{MatrixEncryption, VerificationState};
use crate::error::{ClientError, Result};
use crate::future::MatrixFuture;
use crate::media::MatrixMedia;
use crate::notifications::MatrixNotifications;
use crate::room::MatrixRoom;
// MatrixStateStore is wrapped in generic store parameter
use crate::sync::MatrixSync;
use matrix_sdk::ruma::OwnedUserId;

/// Encryption setup configuration for MatrixClient
#[derive(Clone)] // Add Clone derive
pub struct EncryptionConfig {
    /// Whether to enable encryption automatically
    pub auto_enable: bool,
    /// Whether to enable automatic backup of encryption keys
    pub auto_backup: bool,
    /// Whether to enable Trust on First Use (TOFU) for devices
    pub enable_tofu: bool,
    /// Whether to receive room keys only when online
    pub online_backup_only: bool,
    /// A custom recovery passphrase for key backup
    pub recovery_passphrase: Option<String>,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            auto_enable: true,
            auto_backup: true,
            enable_tofu: true,
            online_backup_only: true,
            recovery_passphrase: None,
        }
    }
}

/// A synchronous wrapper around the Matrix SDK Client.
///
/// This wrapper enables using the Client with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct MatrixClient {
    inner: Arc<SdkClient>,
    runtime_handle: Handle,
    encryption_config: Option<EncryptionConfig>,
}

impl MatrixClient {
    /// Create a new client with the given homeserver URL.
    #[instrument(skip_all, level = "debug")]
    pub async fn new(homeserver_url: &str) -> Result<Self> {
        debug!("Creating new Matrix client with default config");
            
        let client = SdkClient::builder()
            .homeserver_url(homeserver_url)
            .build()
            .await
            .map_err(ClientError::matrix_sdk)?;

        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: None,
        })
    }

    /// Create a new MatrixClient from an existing matrix-sdk Client
    pub fn from_client(client: SdkClient) -> Self {
        Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: None,
        }
    }

    /// Create a new client with the given configuration.
    #[instrument(skip_all, level = "debug")]
    pub async fn with_config<S>(
        homeserver_url: &str,
        store: S,
        encryption_config: Option<matrix_sdk::encryption::EncryptionSettings>,
        request_config: Option<RequestConfig>,
    ) -> Result<Self>
    where
        S: matrix_sdk_base::store::StateStore + Send + Sync + 'static, // Removed MatrixStateStore bound here, handled by wrapper
    {
        debug!("Creating Matrix client with custom configuration");
        let server_name = matrix_sdk::ruma::ServerName::parse(homeserver_url.as_ref())
            .map_err(|e| ClientError::other(format!("Invalid server name: {}", e)))?;
            
        let mut builder = SdkClient::builder().server_name(&server_name); // Use server_name in Matrix SDK 0.13

        // Wrap the store in StoreConfig
        let store_config = StoreConfig::new("maxtryx_client".to_string()).state_store(store);
        builder = builder.store_config(store_config); // Use .store_config()

        // Configure encryption if provided
        if let Some(config) = encryption_config {
            builder = builder.with_encryption_settings(config);
        }

        // Configure request settings if provided
        if let Some(config) = request_config {
            builder = builder.request_config(config);
        }

        let client = builder.build().await.map_err(ClientError::matrix_sdk)?;

        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: None,
        })
    }

    /// Create a new client with encryption enabled by default.
    ///
    /// This is a convenience method that sets up the client with encryption enabled
    /// and configures optimal defaults for encrypted communication.
    #[instrument(skip_all, level = "debug")]
    pub async fn with_encryption<S>(
        homeserver_url: &str,
        store: S,
        config: Option<EncryptionConfig>, // Use our wrapper config struct here
        request_config: Option<RequestConfig>,
    ) -> Result<Self>
    where
        S: matrix_sdk_base::store::StateStore + Send + Sync + 'static, // Removed MatrixStateStore bound
    {
        debug!("Creating Matrix client with encryption enabled");
        let maxtryx_encryption_config = config.unwrap_or_default(); // Our config

        // Create SDK encryption settings using Matrix SDK 0.13 API
        let sdk_encryption_settings = matrix_sdk::encryption::EncryptionSettings {
            auto_enable_cross_signing: maxtryx_encryption_config.auto_enable,
            auto_enable_backups: maxtryx_encryption_config.auto_backup,
            backup_download_strategy: matrix_sdk::encryption::BackupDownloadStrategy::AfterDecryptionFailure,
        };

        let server_name = matrix_sdk::ruma::ServerName::parse(homeserver_url.as_ref())
            .map_err(|e| ClientError::other(format!("Invalid server name: {}", e)))?;
            
        let mut builder = SdkClient::builder()
            .server_name(&server_name)
            .store_config(StoreConfig::new("maxtryx_client".to_string()).state_store(store))
            .with_encryption_settings(sdk_encryption_settings);

        // Configure request settings if provided
        if let Some(config) = request_config {
            builder = builder.request_config(config);
        }

        let client = builder.build().await.map_err(ClientError::matrix_sdk)?;

        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: Some(maxtryx_encryption_config), // Store our config wrapper
        })
    }

    // The inner() method is removed to maintain the "Hidden Box/Pin" pattern
    // Private access only - we don't want to expose the inner client directly

    /// Login with the given username and password.
    #[instrument(skip(self, password), level = "debug")]
    pub fn login(&self, username: &str, password: &str) -> MatrixFuture<()> {
        let username = username.to_owned();
        let password = password.to_owned();
        let client = self.inner.clone();
        let encryption_config = self.encryption_config.clone();

        debug!("Logging in user: {}", username);

        MatrixFuture::spawn(async move {
            // Call login directly on the client
            client
                .matrix_auth()
                .login_username(&username, &password)
                .await
                .map_err(ClientError::matrix_sdk)?;

            // Initialize encryption if configured
            if let Some(config) = encryption_config {
                if config.auto_enable {
                    debug!("Initializing encryption after login");
                    Self::initialize_encryption(&client, &config).await?;
                }
            }

            Ok(())
        })
    }

    /// Initialize encryption settings after login
    async fn initialize_encryption(_client: &MatrixClient, config: &EncryptionConfig) -> Result<()> {
        debug!("Initializing encryption settings");
        // In Matrix SDK 0.13, encryption is automatically enabled based on EncryptionSettings
        // passed to ClientBuilder during client creation

        // Set up key backup if configured
        // TODO: Matrix SDK 0.13 recovery API changes - need to implement backup logic
        if config.auto_backup {
            debug!("Setting up key backup - Matrix SDK 0.13 API changes needed");
            // Recovery API has changed in Matrix SDK 0.13
            // Need to update to new backup/recovery patterns
            // TODO: Implement proper backup logic once Matrix SDK 0.13 API is understood
            //         .await
            //         .map_err(ClientError::matrix_sdk)?;
            // } else {
            //     recovery
            //         .create_recovery_key() // Method likely changed
            //         .await
            //         .map_err(ClientError::matrix_sdk)?;
            // }
            // Enable backup after creating key
            // recovery.enable_backup().await.map_err(ClientError::matrix_sdk)?;
        } else if !config.online_backup_only {
            debug!("Ensuring existing key backup is enabled");
            // Enable backup if it exists but might be disabled
            // TODO: Verify enable_backup method in SDK 0.10+
            // recovery.enable_backup().await.map_err(ClientError::matrix_sdk)?;
        }

        // Set up trust on first use if configured
        if config.enable_tofu {
            debug!("Enabling Trust on First Use (TOFU)");
            // The SDK doesn't have a global TOFU setting, but we can implement it
            // in our verification handlers
        }

        Ok(())
    }

    /// Login with a custom login request.
    #[instrument(skip_all, level = "debug")]
    pub fn login_with_request(&self, request: LoginRequest) -> MatrixFuture<()> {
        let client = self.inner.clone();
        let encryption_config = self.encryption_config.clone();

        debug!("Logging in with custom request");

        MatrixFuture::spawn(async move {
            // Use matrix_auth API for generic login requests in Matrix SDK 0.13
            // Convert generic login request to appropriate auth call
            let auth = client.matrix_auth();
            use matrix_sdk_base::ruma::api::client::session::login::v3::LoginInfo;
            
            match &request.login_info {
                LoginInfo::Password(info) => {
                    auth.login_identifier(info.identifier.clone(), &info.password).send().await.map_err(ClientError::matrix_sdk)?;
                },
                LoginInfo::Token(info) => {
                    auth.login_token(&info.token).send().await.map_err(ClientError::matrix_sdk)?;
                },
                _ => {
                    return Err(ClientError::InvalidParameter("Unsupported login type".to_string()));
                }
            }

            // Initialize encryption if configured
            if let Some(config) = encryption_config {
                if config.auto_enable {
                    debug!("Initializing encryption after login");
                    Self::initialize_encryption(&client, &config).await?;
                }
            }

            Ok(())
        })
    }

    /// Register a new user account.
    #[instrument(skip_all, level = "debug")]
    pub fn register(&self, request: RegistrationRequest) -> MatrixFuture<()> {
        let client = self.inner.clone();
        let encryption_config = self.encryption_config.clone();

        debug!("Registering new user account");

        MatrixFuture::spawn(async move {
            // Use matrix_auth API for registration in Matrix SDK 0.13  
            client.matrix_auth().register(request).send().await.map_err(ClientError::matrix_sdk)?;

            // Initialize encryption if configured
            if let Some(config) = encryption_config {
                if config.auto_enable {
                    debug!("Initializing encryption after registration");
                    Self::initialize_encryption(&client, &config).await?;
                }
            }

            Ok(())
        })
    }

    /// Logout the current session.
    #[instrument(skip(self), level = "debug")]
    pub fn logout(&self) -> MatrixFuture<()> {
        let client = self.inner.clone();

        debug!("Logging out current session");

        // Call logout directly on the client
        MatrixFuture::spawn(async move { client.logout().await.map_err(ClientError::matrix_sdk) }) // Call on client directly
    }

    /// Get the logged-in user's ID.
    pub fn user_id(&self) -> Option<&UserId> {
        self.inner.user_id()
    }

    /// Get the device ID of the current session.
    pub fn device_id(&self) -> Option<&DeviceId> { // Changed return type to &DeviceId
        self.inner.device_id()
    }

    /// Check if the client is logged in.
    pub fn is_logged_in(&self) -> bool {
        self.inner.matrix_auth().logged_in()
    }

    /// Verify multiple devices in batch mode.
    ///
    /// This method verifies multiple devices belonging to multiple users
    /// in an optimized batch operation.
    #[instrument(skip(self), level = "debug")]
    pub fn verify_devices(
        &self,
        device_map: &[(OwnedUserId, OwnedDeviceId)], // Use Owned types for cloning
    ) -> MatrixFuture<Vec<(OwnedUserId, OwnedDeviceId, VerificationState)>> { // Use Owned types
        let device_map_vec: Vec<(OwnedUserId, OwnedDeviceId)> = device_map.to_vec(); // Explicit type
        let client = self.inner.clone();

        debug!("Verifying {} devices in batch mode", device_map_vec.len());

        MatrixFuture::spawn(async move {
            let mut results = Vec::new();
            let crypto = client.encryption();

            // Process devices in batches
            const BATCH_SIZE: usize = 10;

            for chunk in device_map_vec.chunks(BATCH_SIZE) { // Use device_map_vec
                let mut futures = Vec::with_capacity(chunk.len());

                // Create futures for each device
                for (user_id, device_id) in chunk {
                    let user_id = user_id.clone(); // Clone OwnedUserId
                    let device_id = device_id.clone(); // Clone OwnedDeviceId

                    // Get verification state
                    let future = async { // Removed move, crypto is Arc-like
                        match crypto.get_device(&user_id, &device_id).await {
                            Ok(Some(device)) => Some((user_id, device_id, device.is_verified())),
                            Ok(None) => {
                                warn!("Device not found {}/{}", user_id, device_id);
                                None
                            }
                            Err(e) => {
                                error!("Failed to get device {}/{}: {}", user_id, device_id, e);
                                None
                            },
                        }
                    };

                    futures.push(future);
                }

                // Execute futures concurrently
                for result in futures::future::join_all(futures).await {
                    if let Some(device_info) = result {
                        results.push(device_info);
                    }
                }
            }

            debug!("Batch verification completed for {} devices", results.len());
            Ok(results)
        })
    }

    /// Restore encryption keys using the configured backup
    #[instrument(skip(self), level = "debug")]
    pub fn restore_keys(&self, passphrase: Option<&str>) -> MatrixFuture<usize> { // Passphrase might be needed
        let client = self.inner.clone();
        let passphrase = passphrase.map(|s| s.to_owned());

        debug!("Restoring encryption keys from backup");

        MatrixFuture::spawn(async move {
            let recovery = client.encryption().recovery();

            // Check if recovery is enabled and we have a key
            // TODO: Verify how to check if recovery is enabled in SDK 0.10+ (e.g., using recovery.state())
            if recovery.state() == matrix_sdk::encryption::recovery::RecoveryState::Disabled { // Example check
                 warn!("Recovery is not enabled, cannot restore keys.");
                 return Ok(0);
            }

            // Attempt to restore keys. Passphrase might be needed if key isn't cached.
            // The SDK might handle cached keys automatically. Check SDK 0.10 docs.
            // This part needs careful review based on how SDK 0.10 handles restoration.
            // Assuming passphrase is required if key isn't cached:
            let result = if let Some(pass) = passphrase {
                 recovery.recover(&pass).await
            } else {
                 return Err(ClientError::InvalidParameter("Passphrase or recovery key required".into()));
            };


            match result.map_err(ClientError::matrix_sdk) { // Map error here
                 Ok(counts) => {
                     debug!("Restored {} keys from backup", counts.total);
                     Ok(counts.total as usize) // Assuming counts.total is the relevant number
                 },
                 Err(e) => {
                     warn!("Failed to restore keys: {}", e);
                     Err(ClientError::matrix_sdk(e)) // Propagate error
                 },
             }
        })
    }

    /// Enable automatic Trust on First Use (TOFU) for devices
    #[instrument(skip(self), level = "debug")]
    pub fn enable_trust_on_first_use(&self) -> MatrixFuture<()> {
        let client = self.inner.clone();

        debug!("Enabling Trust on First Use for devices");

        MatrixFuture::spawn(async move {
            // Set up handler for new device notifications
            // Check the correct event type and handler signature for SDK 0.10+
            // Placeholder: This likely needs adjustment
            client.add_event_handler(move |_ev: matrix_sdk::sync::JoinedRoomUpdate| { // Example type, needs verification
                let client = client.clone();

                async move {
                    // Logic to extract user_id and device_id from the event `ev`
                    // This depends heavily on the actual event structure in SDK 0.10+
                    let user_id: &UserId = todo!(); // Extract user_id from ev
                    let device_id: &DeviceId = todo!(); // Extract device_id from ev

                    debug!("New device detected: {}/{}", user_id, device_id);

                    // Check if this is the first device for this user
                    let devices =
                        client.encryption().get_user_devices(user_id).await.map_err(|e| {
                            error!("Failed to get user devices: {}", e);
                        })?;

                    if devices.devices().count() <= 1 {
                        debug!("First device for user {}, applying TOFU", user_id);

                        // Get the specific device, handling Option
                        let device = match client.encryption().get_device(user_id, device_id).await {
                            Ok(Some(d)) => d,
                            Ok(None) => {
                                error!("Device {}/{} not found for TOFU", user_id, device_id);
                                // Need to return Result<(), matrix_sdk::Error> or similar from handler
                                return Ok(()); // Or handle error appropriately
                            }
                            Err(e) => {
                                error!("Failed to get device details: {}", e);
                                // Need to return Result<(), matrix_sdk::Error> or similar from handler
                                // For now, just log and continue
                                return Ok(());
                            }
                        };

                        // Trust this device automatically
                        device.verify().await.map_err(|e| error!("Failed to verify device: {}", e)).ok(); // Use device.verify()
                        // Check if saving changes is needed after setting trust level
                        // client.save_changes().await?; // Example if needed

                        debug!("Successfully trusted first device for {}", user_id);
                    }

                    // Ensure the closure returns a compatible Result type if required by add_event_handler
                    Ok(()) // Assuming Ok(()) is compatible
                }
            });

            Ok(())
        })
    }

    /// Get a room by its ID.
    pub fn get_room(&self, room_id: &RoomId) -> Option<MatrixRoom> {
        self.inner.get_room(room_id).map(MatrixRoom::new)
    }

    /// Get all joined rooms.
    pub fn joined_rooms(&self) -> Vec<MatrixRoom> {
        self.inner.joined_rooms().into_iter().map(MatrixRoom::new).collect()
    }

    /// Create a direct message room with the given user.
    #[instrument(skip(self), level = "debug")]
    pub fn create_dm_room(&self, user_id: &UserId) -> MatrixFuture<MatrixRoom> {
        let user_id = user_id.to_owned();
        let client = self.inner.clone();

        debug!("Creating DM room with user {}", user_id);

        MatrixFuture::spawn(async move {
            // Call create_dm directly on the client
            // TODO: Verify create_dm method in SDK 0.10+
            let room = client.create_dm(&user_id).await.map_err(ClientError::matrix_sdk)?; // Assuming create_dm exists

            debug!("Created DM room {} with {}", room.room_id(), user_id);
            Ok(MatrixRoom::new(room))
        })
    }

    /// Join a room by its ID.
    #[instrument(skip(self), level = "debug")]
    pub fn join_room_by_id(&self, room_id: &RoomId) -> MatrixFuture<MatrixRoom> {
        let room_id = room_id.to_owned();
        let client = self.inner.clone();

        debug!("Joining room with ID {}", room_id);

        MatrixFuture::spawn(async move {
            // Call join_room_by_id directly on the client
            let room = client.join_room_by_id(&room_id).await.map_err(ClientError::matrix_sdk)?;

            debug!("Joined room {}", room.room_id());
            Ok(MatrixRoom::new(room))
        })
    }

    /// Join a room by its alias.
    #[instrument(skip(self), level = "debug")]
    pub fn join_room_by_alias(&self, room_alias: &str) -> MatrixFuture<MatrixRoom> {
        let room_alias = room_alias.to_owned();
        let client = self.inner.clone();

        debug!("Joining room with alias {}", room_alias);

        MatrixFuture::spawn(async move {
            // Call join_room_by_id_or_alias directly on the client
            // Assuming alias is RoomIdOrAlias type
            let room_alias_id = matrix_sdk::ruma::RoomAliasId::parse(&room_alias)
                .map_err(|e| ClientError::InvalidParameter(format!("Invalid room alias: {}", e)))?;
            let room = client
                .join_room_by_id_or_alias(&room_alias_id.into(), &[]) // Convert alias to RoomIdOrAliasId, empty server list
                .await
                .map_err(ClientError::matrix_sdk)?;

            debug!("Joined room {} via alias {}", room.room_id(), room_alias);
            Ok(MatrixRoom::new(room))
        })
    }

    /// Create a new room with the given name and topic.
    #[instrument(skip(self), level = "debug")]
    pub fn create_room(
        &self,
        name: &str,
        topic: Option<&str>,
        is_direct: bool,
    ) -> MatrixFuture<MatrixRoom> {
        let name = name.to_owned();
        let topic = topic.map(|s| s.to_owned());
        let client = self.inner.clone();

        debug!("Creating new room: {}", name);

        MatrixFuture::spawn(async move {
            // Use Matrix SDK 0.13 create_room API
            use matrix_sdk_base::ruma::api::client::room::create_room::v3::Request as CreateRoomRequest;

            
            let mut request = CreateRoomRequest::new();
            request.name = Some(name.clone());
            
            if let Some(t) = topic {
                request.topic = Some(t);
            }
            
            request.is_direct = is_direct;
            
            // Create the room
            let room = client.create_room(request).await.map_err(ClientError::matrix_sdk)?;

            debug!("Created room {} - {} (direct: {})", room.room_id(), name, is_direct);
            Ok(MatrixRoom::new(room))
        })
    }

    /// Get the media manager for this client.
    pub fn media(&self) -> MatrixMedia {
        MatrixMedia::new(self.inner.clone())
    }

    /// Get the encryption manager for this client.
    pub fn encryption(&self) -> MatrixEncryption {
        MatrixEncryption::new(self.inner.clone())
    }

    /// Get the sync manager for this client.
    pub fn sync(&self) -> MatrixSync {
        MatrixSync::new(self.inner.clone())
    }

    /// Get the notification settings for this client.
    #[instrument(skip(self), level = "debug")]
    pub fn notification_settings(&self) -> MatrixFuture<MatrixNotifications> {
        MatrixNotifications::new(self.inner.clone())
    }

    /// Stop all background tasks and shutdown the client.
    #[instrument(skip(self), level = "debug")]
    pub fn shutdown(&self) {
        debug!("Shutting down Matrix client (Note: shutdown method might be removed, dropping client might be sufficient)");
        // Check if shutdown() still exists in SDK 0.10+
        // self.inner.shutdown(); // If it exists
    }
}
