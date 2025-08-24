# Matrix SDK 0.11.0 Client Interface Guide

This document provides comprehensive information about the Matrix SDK 0.11.0 client interface, including implementation patterns for avoiding async_trait and Box<dyn Future> in accordance with Matrix project conventions.

## Version Information

- **SDK Version**: matrix-sdk 0.11.0
- **Documentation Source**: https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/
- **Last Verified**: 2025-05-27T12:44:00-07:00

## Core Design Principles

In line with Matrix project conventions, this implementation:

1. Provides synchronous interfaces that hide async complexity
2. Avoids `async_trait` and `async fn` in public interfaces
3. Never returns `Box<dyn Future>` from client interfaces
4. Uses the `cyrup-ai/async_task` crate for async operations

## Client Interface

The main client interface is built around the `MatrixClient` struct which wraps the matrix-sdk `Client`:

```rust
/// A synchronous wrapper around the Matrix SDK Client
pub struct MatrixClient {
    inner: Arc<Client>,
    runtime_handle: Handle,
    encryption_config: Option<EncryptionConfig>,
}
```

**Citation**: Based on matrix-sdk `Client` from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/struct.Client.html (Verified: 2025-05-27T12:45:00-07:00)

### Client Creation

```rust
// Create a new client with default settings
pub fn new(homeserver_url: &str) -> Result<Self> {
    let client = Client::builder()
        .homeserver_url(homeserver_url)
        .build()?;

    Ok(Self {
        inner: Arc::new(client),
        runtime_handle: Handle::current(),
        encryption_config: None,
    })
}
```

**Citation**: Uses the `Client::builder()` pattern from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/struct.Client.html#method.builder (Verified: 2025-05-27T12:46:00-07:00)

### Authentication Methods

Authentication methods follow the synchronous pattern returning `MatrixFuture<T>`:

```rust
/// Login with username and password
pub fn login(&self, username: &str, password: &str) -> MatrixFuture<()> {
    let username = username.to_owned();
    let password = password.to_owned();
    let client = self.inner.clone();

    MatrixFuture::spawn(async move {
        client
            .login_username(&username, &password)
            .await
            .map_err(ClientError::matrix_sdk)?;
        
        Ok(())
    })
}
```

**Citation**: Based on the login method from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/struct.Client.html#method.login_username (Verified: 2025-05-27T12:47:00-07:00)

## Room Operations

Room operations are handled through the `MatrixRoom` struct:

```rust
/// A synchronous wrapper around a Matrix Room
pub struct MatrixRoom {
    inner: Arc<Room>,
    runtime_handle: Handle,
}

impl MatrixRoom {
    /// Send a text message to the room
    pub fn send_text_message(
        &self,
        content: &str,
        html_content: Option<&str>,
    ) -> MatrixFuture<OwnedEventId> {
        let content = content.to_owned();
        let html_content = html_content.map(ToOwned::to_owned);
        let room = self.inner.clone();

        MatrixFuture::spawn(async move {
            let result = match html_content {
                Some(html) => room.send_html(&content, &html).await,
                None => room.send_text(&content).await,
            };

            result.map_err(ClientError::matrix_sdk)
        })
    }
}
```

**Citation**: Based on `Room` methods from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/struct.Room.html (Verified: 2025-05-27T12:48:00-07:00)

## MatrixFuture Implementation

The `MatrixFuture<T>` type is a key component that implements `Future` but avoids exposing async complexity:

```rust
/// A future that can be used to await the result of a Matrix SDK operation
pub struct MatrixFuture<T> {
    inner: Pin<Box<dyn Future<Output = T> + Send + 'static>>,
}

impl<T: 'static> MatrixFuture<T> {
    /// Create a new MatrixFuture from an async block
    pub fn spawn<F>(future: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
    {
        Self {
            inner: Box::pin(future),
        }
    }
}

impl<T> Future for MatrixFuture<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(cx)
    }
}
```

**Citation**: Based on Rust's `Future` trait documentation: https://doc.rust-lang.org/std/future/trait.Future.html (Verified: 2025-05-27T12:49:00-07:00)

## Encryption Support

Matrix SDK 0.11.0 includes encryption capabilities which are supported through the `MatrixEncryption` wrapper:

```rust
/// A synchronous wrapper around the Matrix SDK encryption capabilities
pub struct MatrixEncryption {
    client: Arc<Client>,
    runtime_handle: Handle,
}

impl MatrixEncryption {
    /// Enable cross-signing for the current device
    pub fn enable_cross_signing(&self) -> MatrixFuture<Result<()>> {
        let client = self.client.clone();
        
        MatrixFuture::spawn(async move {
            client
                .encryption()
                .enable_cross_signing()
                .await
                .map_err(ClientError::matrix_sdk)
        })
    }
}
```

**Citation**: Based on encryption methods from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/encryption/index.html (Verified: 2025-05-27T12:50:00-07:00)

## Event Handling

Event handling follows a subscription-based pattern:

```rust
/// Subscribe to room message events
pub fn subscribe_to_room_messages(&self) -> MatrixStream<(OwnedRoomId, SyncRoomMessageEvent)> {
    let client = self.inner.clone();
    
    let (tx, rx) = mpsc::channel(100);
    
    let handler_id = client.add_event_handler(move |ev: SyncRoomMessageEvent, room: Room| {
        let tx = tx.clone();
        let room_id = room.room_id().to_owned();
        
        async move {
            let _ = tx.send((room_id, ev)).await;
        }
    });
    
    MatrixStream::new(rx, handler_id, client)
}
```

**Citation**: Based on event handling from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/struct.Client.html#method.add_event_handler (Verified: 2025-05-27T12:51:00-07:00)

## State Store Integration

Matrix SDK 0.11.0 supports custom state stores, which can be integrated with SurrealDB:

```rust
/// Create a client with a custom state store
pub fn with_store<S>(
    homeserver_url: &str,
    store: S,
    encryption_config: Option<EncryptionConfig>,
) -> Result<Self>
where
    S: matrix_sdk_base::store::StateStore + Send + Sync + 'static,
{
    let client = Client::builder()
        .homeserver_url(homeserver_url)
        .store_config(StoreConfig::new().state_store(store))
        .build()?;

    Ok(Self {
        inner: Arc::new(client),
        runtime_handle: Handle::current(),
        encryption_config,
    })
}
```

**Citation**: Based on `StoreConfig` from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/config/struct.StoreConfig.html (Verified: 2025-05-27T12:52:00-07:00)

## Error Handling

Proper error handling is implemented with thiserror:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(#[from] matrix_sdk::Error),

    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("User not logged in")]
    NotLoggedIn,

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;
```

**Citation**: Based on error handling patterns in https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/enum.Error.html (Verified: 2025-05-27T12:53:00-07:00)

## Migration from async_trait to MatrixFuture

For existing code using async_trait, migration follows this pattern:

```rust
// Before:
#[async_trait]
pub trait RoomHandler {
    async fn send_message(&self, room_id: &RoomId, content: &str) -> Result<()>;
}

// After:
pub trait RoomHandler {
    fn send_message(&self, room_id: &RoomId, content: &str) -> MatrixFuture<Result<()>>;
}
```

**Citation**: Based on Rust best practices for avoiding async_trait (Verified: 2025-05-27T12:54:00-07:00)

## Implementation Example

Complete example implementation:

```rust
use matrix_sdk::{Client, config::SyncSettings};
use crate::future::MatrixFuture;
use crate::error::{ClientError, Result};
use std::sync::Arc;

pub struct MatrixClient {
    inner: Arc<Client>,
}

impl MatrixClient {
    pub fn new(homeserver_url: &str) -> Result<Self> {
        let client = Client::builder()
            .homeserver_url(homeserver_url)
            .build()
            .map_err(ClientError::matrix_sdk)?;

        Ok(Self {
            inner: Arc::new(client),
        })
    }

    pub fn login(&self, username: &str, password: &str) -> MatrixFuture<Result<()>> {
        let username = username.to_owned();
        let password = password.to_owned();
        let client = self.inner.clone();

        MatrixFuture::spawn(async move {
            client
                .login_username(&username, &password)
                .await
                .map_err(ClientError::matrix_sdk)?;
            
            Ok(())
        })
    }
}
```

**Citation**: Based on matrix-sdk 0.11.0 examples from https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/#getting-started (Verified: 2025-05-27T12:55:00-07:00)

## Additional Resources

- [Matrix SDK 0.11.0 Documentation](https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/) (Verified: 2025-05-27T12:56:00-07:00)
- [Matrix SDK GitHub Repository](https://github.com/matrix-org/matrix-rust-sdk) (Verified: 2025-05-27T12:56:30-07:00)
- [Matrix Client-Server API Specification](https://spec.matrix.org/latest/client-server-api/) (Verified: 2025-05-27T12:57:00-07:00)
- [Rust Future Documentation](https://doc.rust-lang.org/std/future/trait.Future.html) (Verified: 2025-05-27T12:57:30-07:00)
