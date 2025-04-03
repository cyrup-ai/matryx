//! Matrix client wrapper with synchronous interfaces that hide async complexity
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Client
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;
use tracing::{debug, error, info, instrument, trace, warn};

use matrix_sdk::{
    config::RequestConfig,
    encryption::{
        verification::VerificationState,
        BackupDownloadStrategy,
        EncryptionEnabled,
        EncryptionSettings,
        TrustLevel,
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
    Client as MatrixClient,
    ClientBuilder,
};

use crate::encryption::CyrumEncryption;
use crate::error::{ClientError, Result};
use crate::future::{MatrixFuture, MatrixStream};
use crate::media::CyrumMedia;
use crate::notifications::CyrumNotifications;
use crate::room::CyrumRoom;
use crate::store::CyrumStateStore;
use crate::sync::CyrumSync;

/// Encryption setup configuration for CyrumClient
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
        let client = ClientBuilder::new()
            .homeserver_url(homeserver_url)
            .build()
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
        encryption_settings: Option<EncryptionSettings>,
        request_config: Option<RequestConfig>,
    ) -> Result<Self>
    where
        S: matrix_sdk_base::store::StateStore + CyrumStateStore + Send + Sync + 'static,
    {
        debug!("Creating Matrix client with custom configuration");
        let mut builder = ClientBuilder::new().homeserver_url(homeserver_url);

        // Use the store directly
        builder = builder.state_store(store);

        // Configure encryption if provided
        if let Some(settings) = encryption_settings {
            builder = builder.encryption_settings(settings);
        }

        // Configure request settings if provided
        if let Some(config) = request_config {
            builder = builder.request_config(config);
        }

        let client = builder.build().map_err(ClientError::matrix_sdk)?;

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
        config: Option<EncryptionConfig>,
        request_config: Option<RequestConfig>,
    ) -> Result<Self>
    where
        S: matrix_sdk_base::store::StateStore + CyrumStateStore + Send + Sync + 'static,
    {
        debug!("Creating Matrix client with encryption enabled");
        let encryption_config = config.unwrap_or_default();

        // Create encryption settings with optimal defaults
        let encryption_settings = EncryptionSettings::default()
            .disable_backups_on_startup(!encryption_config.auto_backup);

        let mut builder = ClientBuilder::new()
            .homeserver_url(homeserver_url)
            .state_store(store)
            .encryption_settings(encryption_settings);

        // Configure request settings if provided
        if let Some(config) = request_config {
            builder = builder.request_config(config);
        }

        let client = builder.build().map_err(ClientError::matrix_sdk)?;

        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config: Some(encryption_config),
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
            client
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
        if client.encryption().is_enabled() == EncryptionEnabled::NoE2EE {
            debug!("Enabling encryption");
            client.encryption().enable().await.map_err(ClientError::matrix_sdk)?;
        }

        // Set up key backup if configured
        if config.auto_backup {
            debug!("Setting up key backup");
            let backup_exists = client
                .encryption()
                .backup_info()
                .await
                .map_err(ClientError::matrix_sdk)?
                .is_some();

            if !backup_exists {
                debug!("Creating new key backup");
                if let Some(passphrase) = &config.recovery_passphrase {
                    client
                        .encryption()
                        .create_backup_with_passphrase(passphrase)
                        .await
                        .map_err(ClientError::matrix_sdk)?;
                } else {
                    client
                        .encryption()
                        .create_backup_version()
                        .await
                        .map_err(ClientError::matrix_sdk)?;
                }
            } else if !config.online_backup_only {
                debug!("Enabling existing key backup");
                if let Some(info) =
                    client.encryption().backup_info().await.map_err(ClientError::matrix_sdk)?
                {
                    client
                        .encryption()
                        .enable_backup_v1(info.version)
                        .await
                        .map_err(ClientError::matrix_sdk)?;
                }
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
            client.login(request).await.map_err(ClientError::matrix_sdk)?;

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
            client.register(request).await.map_err(ClientError::matrix_sdk)?;

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

        MatrixFuture::spawn(async move { client.logout().await.map_err(ClientError::matrix_sdk) })
    }

    /// Get the logged-in user's ID.
    pub fn user_id(&self) -> Option<&UserId> {
        self.inner.user_id()
    }

    /// Get the device ID of the current session.
    pub fn device_id(&self) -> Option<&OwnedDeviceId> {
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
        device_map: &[(UserId, DeviceId)],
    ) -> MatrixFuture<Vec<(UserId, DeviceId, VerificationState)>> {
        let device_map = device_map.to_vec();
        let client = self.inner.clone();

        debug!("Verifying {} devices in batch mode", device_map.len());

        MatrixFuture::spawn(async move {
            let mut results = Vec::new();
            let crypto = client.encryption();

            // Process devices in batches
            const BATCH_SIZE: usize = 10;

            for chunk in device_map.chunks(BATCH_SIZE) {
                let mut futures = Vec::with_capacity(chunk.len());

                // Create futures for each device
                for (user_id, device_id) in chunk {
                    let user_id = user_id.clone();
                    let device_id = device_id.clone();

                    // Get verification state
                    let future = async move {
                        match crypto.get_device(&user_id, &device_id).await {
                            Ok(device) => Some((user_id, device_id, device.verification_state())),
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
    pub fn restore_keys(&self) -> MatrixFuture<usize> {
        let client = self.inner.clone();

        debug!("Restoring encryption keys from backup");

        MatrixFuture::spawn(async move {
            // Get backup info
            let backup_info =
                client.encryption().backup_info().await.map_err(ClientError::matrix_sdk)?;

            let backup_info = match backup_info {
                Some(info) => info,
                None => {
                    warn!("No backup info available");
                    return Ok(0);
                },
            };

            // Try to restore with existing key in store
            match client
                .encryption()
                .restore_backup_with_cached_key(
                    backup_info.version,
                    BackupDownloadStrategy::LazyLoadRoomKeys,
                )
                .await
            {
                Ok(result) => {
                    debug!("Restored {} keys from backup with cached key", result.imported_count);
                    Ok(result.imported_count)
                },
                Err(e) => {
                    warn!("Failed to restore with cached key: {}", e);
                    // Cache key is invalid or not available
                    Ok(0)
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
            client.add_event_handler(move |ev: matrix_sdk::events::DeviceEvent| {
                let client = client.clone();

                async move {
                    let user_id = &ev.sender;
                    let device_id = &ev.content.device_id;

                    debug!("New device detected: {}/{}", user_id, device_id);

                    // Check if this is the first device for this user
                    let devices =
                        client.encryption().get_user_devices(user_id).await.map_err(|e| {
                            error!("Failed to get user devices: {}", e);
                        })?;

                    if devices.devices().count() <= 1 {
                        debug!("First device for user {}, applying TOFU", user_id);

                        // Get the specific device
                        let device =
                            client.encryption().get_device(user_id, device_id).await.map_err(
                                |e| {
                                    error!("Failed to get device details: {}", e);
                                },
                            )?;

                        // Trust this device automatically
                        device.set_trust_level(TrustLevel::Trusted).await.map_err(|e| {
                            error!("Failed to trust device: {}", e);
                        })?;

                        debug!("Successfully trusted first device for {}", user_id);
                    }

                    Ok(())
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
            let room = client.create_dm_room(&user_id).await.map_err(ClientError::matrix_sdk)?;

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
            let room = client
                .join_room_by_alias(&room_alias)
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
            let mut request = matrix_sdk::ruma::api::client::room::create_room::v3::Request::new();
            request.name = Some(name.clone());
            request.topic = topic.clone();
            request.is_direct = is_direct;

            let room = client.create_room(request).await.map_err(ClientError::matrix_sdk)?;

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
        debug!("Shutting down Matrix client");
        self.inner.shutdown();
    }
}
