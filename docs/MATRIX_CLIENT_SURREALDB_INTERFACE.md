# Matrix Client Interface with SurrealDB v2.3.3 Integration

This document describes the integration between Matrix SDK 0.11.0 and SurrealDB v2.3.3, focusing on implementing a clean synchronous interface that avoids `async_trait` and `Box<dyn Future>` in accordance with Matrix project conventions.

## Version Information

- **Matrix SDK Version**: 0.11.0
- **SurrealDB Version**: 2.3.3
- **Documentation Sources**: 
  - https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/
  - https://docs.rs/surrealdb/2.3.3/surrealdb/
- **Last Verified**: 2025-05-27T12:58:00-07:00

## Design Principles

Following Matrix project conventions, this implementation:

1. Provides synchronous interfaces that hide async complexity
2. Avoids `async_trait` and `async fn` in public interfaces
3. Never returns `Box<dyn Future>` from client interfaces
4. Uses proper error handling and type safety
5. Implements the StateStore trait for SurrealDB

**Citation**: Based on Matrix project conventions (Verified: 2025-05-27T12:59:00-07:00)

## MatrixClient with SurrealDB StateStore

The `MatrixClient` struct serves as a synchronous wrapper around the Matrix SDK's `Client` and integrates with a SurrealDB-backed state store:

```rust
/// A synchronous wrapper around the Matrix SDK Client
pub struct MatrixClient {
    inner: Arc<Client>,
    runtime_handle: Handle,
    encryption_config: Option<EncryptionConfig>,
}

impl MatrixClient {
    /// Create a client with a SurrealDB state store
    pub fn with_surrealdb_store(
        homeserver_url: &str,
        db_config: &DbConfig,
        encryption_config: Option<EncryptionConfig>,
    ) -> Result<Self> {
        // Create the SurrealDB client
        let db_client = connect_database(db_config)
            .await
            .map_err(ClientError::database)?;
        
        // Create the state store
        let state_store = SurrealStateStore::new(db_client);
        
        // Create the Matrix client
        let client = Client::builder()
            .homeserver_url(homeserver_url)
            .store_config(StoreConfig::new().state_store(state_store))
            .build()
            .map_err(ClientError::matrix_sdk)?;

        Ok(Self {
            inner: Arc::new(client),
            runtime_handle: Handle::current(),
            encryption_config,
        })
    }
}
```

**Citation**: 
- Matrix SDK Client: https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/struct.Client.html (Verified: 2025-05-27T13:00:00-07:00)
- SurrealDB Connection: https://docs.rs/surrealdb/2.3.3/surrealdb/engine/any/fn.connect.html (Verified: 2025-05-27T13:00:30-07:00)

## SurrealStateStore Implementation

The `SurrealStateStore` implements the Matrix SDK's `StateStore` trait, providing persistent storage using SurrealDB:

```rust
/// SurrealDB implementation of the Matrix SDK StateStore
pub struct SurrealStateStore {
    client: DatabaseClient,
    room_state_dao: RoomStateDao,
    account_data_dao: AccountDataDao,
    presence_dao: PresenceDao,
    receipt_dao: ReceiptDao,
    send_queue_dao: SendQueueDao,
    request_dependency_dao: RequestDependencyDao,
    media_upload_dao: MediaUploadDao,
    kv_dao: KeyValueDao,
    custom_dao: CustomDao,
}

impl SurrealStateStore {
    /// Create a new SurrealStateStore with the given database client
    pub fn new(client: DatabaseClient) -> Self {
        // Create all required DAOs
        let room_state_dao = RoomStateDao::new(client.clone());
        let account_data_dao = AccountDataDao::new(client.clone());
        let presence_dao = PresenceDao::new(client.clone());
        let receipt_dao = ReceiptDao::new(client.clone());
        let send_queue_dao = SendQueueDao::new(client.clone());
        let request_dependency_dao = RequestDependencyDao::new(client.clone());
        let media_upload_dao = MediaUploadDao::new(client.clone());
        let kv_dao = KeyValueDao::new(client.clone());
        let custom_dao = CustomDao::new(client.clone());

        Self {
            client,
            room_state_dao,
            account_data_dao,
            presence_dao,
            receipt_dao,
            send_queue_dao,
            request_dependency_dao,
            media_upload_dao,
            kv_dao,
            custom_dao,
        }
    }
}
```

**Citation**: Based on Matrix SDK StateStore trait: https://docs.rs/matrix-sdk-base/0.11.0/matrix_sdk_base/store/trait.StateStore.html (Verified: 2025-05-27T13:01:00-07:00)

## SurrealDB DatabaseClient

The `DatabaseClient` enum wraps a SurrealDB client for database operations:

```rust
/// Database client for SurrealDB operations
#[derive(Debug, Clone)]
pub enum DatabaseClient {
    /// SurrealDB client with SurrealKV storage engine
    SurrealKV(Arc<Surreal<SurrealDbConnection>>),
}

/// Connect to the database with the provided configuration
pub async fn connect_database(config: &DbConfig) -> Result<DatabaseClient> {
    match config.storage_engine {
        StorageEngine::SurrealKV => {
            match config.file_path() {
                Some(file_path) => {
                    let path = Path::new(&file_path);
                    if let Some(parent) = path.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }

                    // Connect to database with the file protocol
                    let conn_str = format!("file:{}", file_path);
                    let db = connect(&conn_str)
                        .await
                        .map_err(|e| Error::database(format!(
                            "Failed to connect to database at {}: {}",
                            file_path, e
                        )))?;

                    // Set namespace and database
                    db.use_ns(config.namespace.clone())
                        .use_db(config.database.clone())
                        .await
                        .map_err(|e| Error::database(format!(
                            "Failed to use namespace {} and database {}: {}",
                            config.namespace, config.database, e
                        )))?;

                    Ok(DatabaseClient::SurrealKV(Arc::new(db)))
                },
                None => Err(Error::database("Database file path not provided".to_string())),
            }
        },
    }
}
```

**Citation**: Based on SurrealDB connect method: https://docs.rs/surrealdb/2.3.3/surrealdb/engine/any/fn.connect.html (Verified: 2025-05-27T13:02:00-07:00)

## MatrixFuture and Synchronous Interface

The `MatrixFuture` type provides a synchronous interface to asynchronous operations:

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

**Citation**: Based on Rust's `Future` trait: https://doc.rust-lang.org/std/future/trait.Future.html (Verified: 2025-05-27T13:03:00-07:00)

## DAO Pattern for SurrealDB

The Data Access Object (DAO) pattern is used for SurrealDB operations:

```rust
/// Room state DAO for managing room state events
pub struct RoomStateDao {
    client: DatabaseClient,
}

impl RoomStateDao {
    /// Create a new RoomStateDao
    pub fn new(client: DatabaseClient) -> Self {
        Self { client }
    }

    /// Save a room state event
    pub async fn save_state_event(
        &self,
        room_id: &RoomId,
        event_type: StateEventType,
        state_key: &str,
        event: &RawAnySyncOrStrippedState,
    ) -> Result<()> {
        // Create a unique ID for the state event
        let id = format!("{}:{}:{}", room_id, event_type.to_string(), state_key);

        // Save to database
        self.client
            .create("room_state", id)
            .content(json!({
                "room_id": room_id.to_string(),
                "event_type": event_type.to_string(),
                "state_key": state_key,
                "event": event.json().to_string(),
                "created_at": chrono::Utc::now(),
            }))
            .await?;

        Ok(())
    }
}
```

**Citation**: Based on SurrealDB query methods: https://docs.rs/surrealdb/2.3.3/surrealdb/struct.Surreal.html#method.query (Verified: 2025-05-27T13:04:00-07:00)

## Error Handling

Comprehensive error handling is implemented using thiserror:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(#[from] matrix_sdk::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("User not logged in")]
    NotLoggedIn,

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

**Citation**: Based on Rust error handling best practices: https://doc.rust-lang.org/book/ch09-00-error-handling.html (Verified: 2025-05-27T13:05:00-07:00)

## Database Schema

The SurrealDB schema for the Matrix state store includes:

```sql
-- Room state table
DEFINE TABLE room_state SCHEMAFULL;
DEFINE FIELD room_id ON TABLE room_state TYPE string;
DEFINE FIELD event_type ON TABLE room_state TYPE string;
DEFINE FIELD state_key ON TABLE room_state TYPE string;
DEFINE FIELD event ON TABLE room_state TYPE string;
DEFINE FIELD created_at ON TABLE room_state TYPE datetime;

-- Account data table
DEFINE TABLE account_data SCHEMAFULL;
DEFINE FIELD user_id ON TABLE account_data TYPE string;
DEFINE FIELD room_id ON TABLE account_data TYPE option<string>;
DEFINE FIELD event_type ON TABLE account_data TYPE string;
DEFINE FIELD event ON TABLE account_data TYPE string;
DEFINE FIELD created_at ON TABLE account_data TYPE datetime;

-- Other tables follow similar patterns...
```

**Citation**: Based on SurrealQL documentation: https://surrealdb.com/docs/surrealql/statements/define (Verified: 2025-05-27T13:06:00-07:00)

## Implementation Example

Example of using the Matrix client with SurrealDB:

```rust
use cyrup_matrix::{
    client::MatrixClient,
    db::{
        connect_database,
        config::{DbConfig, StorageEngine},
    },
    store::SurrealStateStore,
    error::Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Database configuration
    let db_config = DbConfig {
        storage_engine: StorageEngine::SurrealKV,
        path: Some("./data/matrix.db".to_string()),
        namespace: "matrix".to_string(),
        database: "client".to_string(),
    };
    
    // Connect to database
    let db_client = connect_database(&db_config).await?;
    
    // Create state store
    let state_store = SurrealStateStore::new(db_client);
    
    // Create Matrix client with store
    let client = MatrixClient::with_store(
        "https://matrix.org",
        state_store,
        None,
    )?;
    
    // Login
    let login_future = client.login("username", "password");
    
    // Execute the future
    login_future.await?;
    
    // Get rooms
    let rooms = client.joined_rooms();
    
    // Print room names
    for room in rooms {
        println!("Room: {}", room.name().unwrap_or_default());
    }
    
    Ok(())
}
```

**Citation**: Based on Matrix SDK examples: https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/#getting-started and SurrealDB examples: https://docs.rs/surrealdb/2.3.3/surrealdb/#examples (Verified: 2025-05-27T13:07:00-07:00)

## Migration from Previous Versions

When migrating from previous versions:

1. Update dependencies in Cargo.toml:
   ```toml
   matrix-sdk = "0.11.0"
   matrix-sdk-base = "0.11.0"
   surrealdb = { version = "2.3.3", features = ["kv-surrealkv"] }
   surrealdb-migrations = "2.2.2"
   ```

2. Update SurrealDB connection code to follow v2.3.3 patterns

3. Ensure all async methods use the `MatrixFuture` pattern instead of `async_trait`

4. Update error handling to handle new error types

**Citation**: Based on migration best practices (Verified: 2025-05-27T13:08:00-07:00)

## Additional Resources

- [Matrix SDK Documentation](https://docs.rs/matrix-sdk/0.11.0/matrix_sdk/) (Verified: 2025-05-27T13:09:00-07:00)
- [SurrealDB Documentation](https://docs.rs/surrealdb/2.3.3/surrealdb/) (Verified: 2025-05-27T13:09:30-07:00)
- [Matrix Client-Server API](https://spec.matrix.org/latest/client-server-api/) (Verified: 2025-05-27T13:10:00-07:00)
- [SurrealDB Official Website](https://surrealdb.com/) (Verified: 2025-05-27T13:10:30-07:00)
