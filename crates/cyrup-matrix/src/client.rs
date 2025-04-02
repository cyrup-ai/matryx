//! Matrix client wrapper with synchronous interfaces that hide async complexity
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Client
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;

use matrix_sdk::{
    Client as MatrixClient, ClientBuilder,
    encryption::EncryptionSettings,
    config::RequestConfig,
    ruma::{
        api::client::{
            account::register::v3::Request as RegistrationRequest,
            session::login::v3::Request as LoginRequest,
        },
        OwnedDeviceId, RoomId, UserId,
    },
};

use crate::error::{Result, ClientError};
use crate::future::MatrixFuture;
use crate::store::CyrumStateStore;
use crate::room::CyrumRoom;
use crate::media::CyrumMedia;
use crate::encryption::CyrumEncryption;
use crate::sync::CyrumSync;
use crate::notifications::CyrumNotifications;

/// A synchronous wrapper around the Matrix SDK Client.
///
/// This wrapper enables using the Client with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct CyrumClient {
    inner: Arc<MatrixClient>,
    runtime_handle: Handle,
}

impl CyrumClient {
    /// Create a new client with the given homeserver URL.
    pub fn new(homeserver_url: &str) -> Result<Self> {
        let client = ClientBuilder::new()
            .homeserver_url(homeserver_url)
            .build()
            .map_err(ClientError::matrix_sdk)?;
            
        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
        })
    }
    
    /// Create a new CyrumClient from an existing matrix-sdk Client
    pub fn from_client(client: MatrixClient) -> Self {
        Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
        }
    }
    
    /// Create a new client with the given configuration.
    pub fn with_config<S>(
        homeserver_url: &str, 
        store: S,
        encryption_settings: Option<EncryptionSettings>,
        request_config: Option<RequestConfig>,
    ) -> Result<Self> 
    where 
        S: matrix_sdk_base::store::StateStore + CyrumStateStore + Send + Sync + 'static
    {
        let mut builder = ClientBuilder::new()
            .homeserver_url(homeserver_url);
            
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
        
        let client = builder.build()
            .map_err(ClientError::matrix_sdk)?;
            
        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
        })
    }
    
    // The inner() method is removed to maintain the "Hidden Box/Pin" pattern
    // Private access only - we don't want to expose the inner client directly
    
    /// Login with the given username and password.
    pub fn login(&self, username: &str, password: &str) -> MatrixFuture<()> {
        let username = username.to_owned();
        let password = password.to_owned();
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            client.login_username(&username, &password).await
                .map_err(ClientError::matrix_sdk)
        })
    }
    
    /// Login with a custom login request.
    pub fn login_with_request(&self, request: LoginRequest) -> MatrixFuture<()> {
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            client.login(request).await
                .map_err(ClientError::matrix_sdk)
        })
    }
    
    /// Register a new user account.
    pub fn register(&self, request: RegistrationRequest) -> MatrixFuture<()> {
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            client.register(request).await
                .map_err(ClientError::matrix_sdk)
        })
    }
    
    /// Logout the current session.
    pub fn logout(&self) -> MatrixFuture<()> {
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            client.logout().await
                .map_err(ClientError::matrix_sdk)
        })
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
    
    /// Get a room by its ID.
    pub fn get_room(&self, room_id: &RoomId) -> Option<CyrumRoom> {
        self.inner.get_room(room_id)
            .map(CyrumRoom::new)
    }
    
    /// Get all joined rooms.
    pub fn joined_rooms(&self) -> Vec<CyrumRoom> {
        self.inner.joined_rooms()
            .into_iter()
            .map(CyrumRoom::new)
            .collect()
    }
    
    /// Create a direct message room with the given user.
    pub fn create_dm_room(&self, user_id: &UserId) -> MatrixFuture<CyrumRoom> {
        let user_id = user_id.to_owned();
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            let room = client.create_dm_room(&user_id).await
                .map_err(ClientError::matrix_sdk)?;
                
            Ok(CyrumRoom::new(room))
        })
    }
    
    /// Join a room by its ID.
    pub fn join_room_by_id(&self, room_id: &RoomId) -> MatrixFuture<CyrumRoom> {
        let room_id = room_id.to_owned();
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            let room = client.join_room_by_id(&room_id).await
                .map_err(ClientError::matrix_sdk)?;
                
            Ok(CyrumRoom::new(room))
        })
    }
    
    /// Join a room by its alias.
    pub fn join_room_by_alias(&self, room_alias: &str) -> MatrixFuture<CyrumRoom> {
        let room_alias = room_alias.to_owned();
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            let room = client.join_room_by_alias(&room_alias).await
                .map_err(ClientError::matrix_sdk)?;
                
            Ok(CyrumRoom::new(room))
        })
    }
    
    /// Create a new room with the given name and topic.
    pub fn create_room(&self, name: &str, topic: Option<&str>, is_direct: bool) -> MatrixFuture<CyrumRoom> {
        let name = name.to_owned();
        let topic = topic.map(|s| s.to_owned());
        let client = self.inner.clone();
        
        MatrixFuture::spawn(async move {
            let mut request = matrix_sdk::ruma::api::client::room::create_room::v3::Request::new();
            request.name = Some(name);
            request.topic = topic;
            request.is_direct = is_direct;
            
            let room = client.create_room(request).await
                .map_err(ClientError::matrix_sdk)?;
                
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
    pub fn notification_settings(&self) -> MatrixFuture<CyrumNotifications> {
        CyrumNotifications::new(self.inner.clone())
    }
    
    /// Stop all background tasks and shutdown the client.
    pub fn shutdown(&self) {
        self.inner.shutdown();
    }
}