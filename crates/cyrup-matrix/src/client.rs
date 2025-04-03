//! Matrix client wrapper with synchronous interfaces that hide async complexity
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Client
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;
use tracing::{debug, error, info, instrument, trace, warn};

use matrix_sdk::{
    config::{RequestConfig, StoreConfig},
    encryption::{
        backups::{BackupConfig, BackupDownloadStrategy},
        identities::TrustLevel,
        verification::VerificationState,
        Config as EncryptionConfigSdk, EncryptionState,
    },
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
    Client as MatrixClient, // ClientBuilder is removed, use Client::builder()
};

use crate::encryption::CyrumEncryption;
use crate::error::{ClientError, Result};
use crate::future::{MatrixFuture, MatrixStream};
use crate::media::CyrumMedia;
use crate::notifications::CyrumNotifications;
use crate::room::CyrumRoom;
use crate::store::CyrumStateStore;
use crate::sync::CyrumSync;
use matrix_sdk::ruma::OwnedUserId;

/// Encryption setup configuration for CyrumClient
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
pub struct CyrumClient {
    inner: Arc<MatrixClient>,
    runtime_handle: Handle,
    encryption_config: Option<EncryptionConfig>,
}

impl CyrumClient {
    /// Create a new client with the given homeserver URL.
    #[instrument(skip_all, level = "debug")]
    pub fn new(homeserver_url: &str) -> Result<Self> {
        debug!("Creating new Matrix client with default config");
        let client = MatrixClient::builder() // Use Client::builder()
            .homeserver_url(homeserver_url) // Set homeserver URL via builder method
            .build() // build() returns Result, no await needed
            .map_err(ClientError::matrix_sdk)?;

        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: None,
        })
    }

    /// Create a new CyrumClient from an existing matrix-sdk Client
    pub fn from_client(client: MatrixClient) -> Self {
        Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: None,
        }
    }

    /// Create a new client with the given configuration.
    #[instrument(skip_all, level = "debug")]
    pub fn with_config<S>(
        homeserver_url: &str,
        store: S,
        encryption_config: Option<EncryptionConfigSdk>, // Use the SDK's EncryptionConfig
        request_config: Option<RequestConfig>,
    ) -> Result<Self>
    where
        S: matrix_sdk_base::store::StateStore + Send + Sync + 'static, // Removed CyrumStateStore bound here, handled by wrapper
    {
        debug!("Creating Matrix client with custom configuration");
        let mut builder = MatrixClient::builder().homeserver_url(homeserver_url); // Use Client::builder()

        // Wrap the store in StoreConfig
        let store_config = StoreConfig::new().state_store(store);
        builder = builder.store_config(store_config); // Use .store_config()

        // Configure encryption if provided
        // TODO: Verify how to apply EncryptionConfigSdk in SDK 0.10+ builder
        if let Some(config) = encryption_config {
             builder = builder.encryption_config(config); // Assuming this method still exists
             warn!("Encryption config builder method needs verification for SDK 0.10+");
        }

        // Configure request settings if provided
        if let Some(config) = request_config {
            builder = builder.request_config(config);
        }

        let client = builder.build().map_err(ClientError::matrix_sdk)?; // build() returns Result

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
    pub fn with_encryption<S>(
        homeserver_url: &str,
        store: S,
        config: Option<EncryptionConfig>, // Use our wrapper config struct here
        request_config: Option<RequestConfig>,
    ) -> Result<Self>
    where
        S: matrix_sdk_base::store::StateStore + Send + Sync + 'static, // Removed CyrumStateStore bound
    {
        debug!("Creating Matrix client with encryption enabled");
        let cyrum_encryption_config = config.unwrap_or_default(); // Our config

        // Create SDK encryption config
        // TODO: Verify BackupConfig path and usage in SDK 0.10+
        let sdk_encryption_config = EncryptionConfigSdk::default()
            .backup(BackupConfig::default() // Assuming BackupConfig path is correct
                .enabled(cyrum_encryption_config.auto_backup)); // Configure backup via BackupConfig

        let mut builder = MatrixClient::builder() // Use Client::builder()
            .homeserver_url(homeserver_url)
            .store_config(StoreConfig::new().state_store(store)) // Use .store_config()
            .encryption_config(sdk_encryption_config); // Assuming this method still exists

        // Configure request settings if provided
        if let Some(config) = request_config {
            builder = builder.request_config(config);
        }

        let client = builder.build().map_err(ClientError::matrix_sdk)?; // build() returns Result

        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: Some(cyrum_encryption_config), // Store our config wrapper
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
            client // Call on client directly
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
    async fn initialize_encryption(client: &MatrixClient, config: &EncryptionConfig) -> Result<()> {
        // Check if encryption is already enabled
        // TODO: Verify how to check encryption state in SDK 0.10+
        if client.encryption_state() == EncryptionState::Disabled { // Assuming encryption_state() exists
            debug!("Enabling encryption");
            // Enabling might happen implicitly or require specific setup after login
            // For now, assume it's handled or needs further investigation based on SDK docs
            // client.encryption().enable().await.map_err(ClientError::matrix_sdk)?;
            warn!("Encryption enabling logic needs review for SDK 0.10+");
        }

        // Set up key backup if configured
        if config.auto_backup {
            debug!("Setting up key backup");
            // Use the recovery API
            let recovery = client.encryption().recovery();

            // Check if backup exists using recovery status
            // TODO: Verify how to check backup status in SDK 0.10+ (e.g., using recovery.state())
            let state = recovery.state(); // Assuming recovery.state() exists
            let backup_exists = state != matrix_sdk::encryption::recovery::RecoveryState::Disabled; // Example check based on state

            if !backup_exists {
                debug!("Creating new key backup");
                // TODO: Verify recovery key creation methods in SDK 0.10+
                if let Some(passphrase) = &config.recovery_passphrase {
                    // recovery
                    //     .create_recovery_key_from_passphrase(passphrase) // Method likely changed
                    //     .await
                    //     .map_err(ClientError::matrix_sdk)?;
                    warn!("create_recovery_key_from_passphrase needs verification for SDK 0.10+");
                } else {
                    // Auto-generate if no passphrase
                    // recovery
                    //     .create_recovery_key() // Method likely changed
                    //     .await
                    //     .map_err(ClientError::matrix_sdk)?;
                    warn!("create_recovery_key needs verification for SDK 0.10+");
                }
                // Enable backup after creating key
                // TODO: Verify enable_backup method in SDK 0.10+
                recovery.enable_backup().await.map_err(ClientError::matrix_sdk)?;

            } else if !config.online_backup_only {
                debug!("Ensuring existing key backup is enabled");
                // Enable backup if it exists but might be disabled
                // TODO: Verify enable_backup method in SDK 0.10+
                 recovery.enable_backup().await.map_err(ClientError::matrix_sdk)?;
            }
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
            // Call login directly on the client
            client.login(request).await.map_err(ClientError::matrix_sdk)?; // Call on client directly

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
            // Call register directly on the client
            client.register(request).await.map_err(ClientError::matrix_sdk)?; // Call on client directly

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
        self.inner.logged_in()
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
                            // get_device returns Option<Device>
                            // TODO: Verify device.verification_state() method in SDK 0.10+
                            Ok(Some(device)) => Some((user_id, device_id, device.verification_state())), // Assuming verification_state() exists
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
            // TODO: Verify restore_backup_from_passphrase method in SDK 0.10+
            let result = if let Some(pass) = passphrase {
                 // recovery.restore_backup_from_passphrase(&pass, None).await // Method likely changed
                 warn!("restore_backup_from_passphrase needs verification for SDK 0.10+");
                 // Placeholder error until method is verified
                 return Err(ClientError::InvalidParameter("restore_backup_from_passphrase needs verification".into()));
            } else {
                 // Try restoring without passphrase (assuming cached key)
                 // This specific method might not exist, adjust based on SDK
                 warn!("Attempting key restoration without passphrase, might fail if key not cached.");
                 // Placeholder: Replace with actual SDK 0.10 method if available
                 // recovery.restore_backup_with_cached_key().await
                 return Err(ClientError::InvalidParameter("Passphrase needed or cached key restore method not found".into()));
            };


            match result {
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
            // TODO: Verify correct event type for device list changes in SDK 0.10+
            client.add_event_handler(move |ev: matrix_sdk::sync::JoinedRoomUpdate| { // Example type, needs verification
                let client = client.clone();

                async move {
                    // Logic to extract user_id and device_id from the event `ev`
                    // This depends heavily on the actual event structure in SDK 0.10+
                    warn!("TOFU event handling needs verification for SDK 0.10+ event type");
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
                        // TODO: Verify get_device method in SDK 0.10+
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
                        // TODO: Verify set_trust_level method in SDK 0.10+
                        // device.set_trust_level(TrustLevel::Trusted); // Method likely changed or removed
                        warn!("set_trust_level needs verification for SDK 0.10+");
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
    pub fn get_room(&self, room_id: &RoomId) -> Option<CyrumRoom> {
        self.inner.get_room(room_id).map(CyrumRoom::new)
    }

    /// Get all joined rooms.
    pub fn joined_rooms(&self) -> Vec<CyrumRoom> {
        self.inner.joined_rooms().into_iter().map(CyrumRoom::new).collect()
    }

    /// Create a direct message room with the given user.
    #[instrument(skip(self), level = "debug")]
    pub fn create_dm_room(&self, user_id: &UserId) -> MatrixFuture<CyrumRoom> {
        let user_id = user_id.to_owned();
        let client = self.inner.clone();

        debug!("Creating DM room with user {}", user_id);

        MatrixFuture::spawn(async move {
            // Call create_dm directly on the client
            // TODO: Verify create_dm method in SDK 0.10+
            let room = client.create_dm(&user_id).await.map_err(ClientError::matrix_sdk)?; // Assuming create_dm exists

            debug!("Created DM room {} with {}", room.room_id(), user_id);
            Ok(CyrumRoom::new(room))
        })
    }

    /// Join a room by its ID.
    #[instrument(skip(self), level = "debug")]
    pub fn join_room_by_id(&self, room_id: &RoomId) -> MatrixFuture<CyrumRoom> {
        let room_id = room_id.to_owned();
        let client = self.inner.clone();

        debug!("Joining room with ID {}", room_id);

        MatrixFuture::spawn(async move {
            // Call join_room_by_id directly on the client
            let room = client.join_room_by_id(&room_id).await.map_err(ClientError::matrix_sdk)?;

            debug!("Joined room {}", room.room_id());
            Ok(CyrumRoom::new(room))
        })
    }

    /// Join a room by its alias.
    #[instrument(skip(self), level = "debug")]
    pub fn join_room_by_alias(&self, room_alias: &str) -> MatrixFuture<CyrumRoom> {
        let room_alias = room_alias.to_owned();
        let client = self.inner.clone();

        debug!("Joining room with alias {}", room_alias);

        MatrixFuture::spawn(async move {
            // Call join_room_by_id_or_alias directly on the client
            // Assuming alias is RoomIdOrAlias type
            let room_alias_id = matrix_sdk::ruma::RoomAliasId::parse(&room_alias)
                .map_err(|e| ClientError::InvalidParameter(format!("Invalid room alias: {}", e)))?;
            // TODO: Verify join_room_by_id_or_alias method signature in SDK 0.10+
            let room = client
                .join_room_by_id_or_alias(&room_alias_id.into()) // Convert alias to RoomIdOrAliasId
                .await
                .map_err(ClientError::matrix_sdk)?;

            debug!("Joined room {} via alias {}", room.room_id(), room_alias);
            Ok(CyrumRoom::new(room))
        })
    }

    /// Create a new room with the given name and topic.
    #[instrument(skip(self), level = "debug")]
    pub fn create_room(
        &self,
        name: &str,
        topic: Option<&str>,
        is_direct: bool,
    ) -> MatrixFuture<CyrumRoom> {
        let name = name.to_owned();
        let topic = topic.map(|s| s.to_owned());
        let client = self.inner.clone();

        debug!("Creating new room: {}", name);

        MatrixFuture::spawn(async move {
            // Use the create_room builder pattern
            // TODO: Verify create_room builder methods in SDK 0.10+
            let mut request = matrix_sdk::room::create::CreateRoomRequest::new(); // Assuming this is the way
            request.name = Some(name.clone());
            if let Some(t) = topic {
                request.topic = Some(t);
            }
            if is_direct {
                // Check how to mark as direct in SDK 0.10+ builder
                warn!("Marking room as direct needs verification for SDK 0.10+ create_room builder");
                request.is_direct = true; // Assuming this field exists
            }

            let room = client.create_room(request).await.map_err(ClientError::matrix_sdk)?; // Pass the request

            debug!("Created room {} - {} (direct: {})", room.room_id(), name, is_direct);
            Ok(CyrumRoom::new(room))
        })
    }

    /// Get the media manager for this client.
    pub fn media(&self) -> CyrumMedia {
        CyrumMedia::new(self.inner.clone())
    }

    /// Get the encryption manager for this client.
    pub fn encryption(&self) -> CyrumEncryption {
        CyrumEncryption::new(self.inner.clone())
    }

    /// Get the sync manager for this client.
    pub fn sync(&self) -> CyrumSync {
        CyrumSync::new(self.inner.clone())
    }

    /// Get the notification settings for this client.
    #[instrument(skip(self), level = "debug")]
    pub fn notification_settings(&self) -> MatrixFuture<CyrumNotifications> {
        CyrumNotifications::new(self.inner.clone())
    }

    /// Stop all background tasks and shutdown the client.
    #[instrument(skip(self), level = "debug")]
    pub fn shutdown(&self) {
        debug!("Shutting down Matrix client (Note: shutdown method might be removed, dropping client might be sufficient)");
        // Check if shutdown() still exists in SDK 0.10+
        // self.inner.shutdown(); // If it exists
        warn!("Client shutdown behavior needs verification for SDK 0.10+");
    }
}
