# SurrealDB Client

A configurable SurrealDB client library with DAO pattern support and multiple storage engine options, providing a unified interface to SurrealDB's powerful multi-model database capabilities.

## Features

- Support for multiple storage engines:
  - Local file-based KV (for desktop apps, development)
  - TiKV distributed storage (for production, clustered deployments)
  - WebSocket connection to remote SurrealDB instances
  - **NEW**: SurrealKV embedded key-value store with versioning support
- Multi-model database capabilities:
  - **Document**: Schema-flexible JSON documents with SurrealQL querying
  - **Graph**: Native graph relationships with edge traversal
  - **Relational**: Table JOINs, indexes, and SQL-like querying
  - **Time-series**: Time-based data with temporal functions
  - **Vector**: Native vector field support for AI and ML applications
- Configuration-based engine selection
- Generic DAO pattern implementation with idiomatic Rust code
- Convenient error handling with custom error types
- Transaction support with ACID guarantees
- Metrics collection for performance monitoring
- Automated migration support

## Usage

### Basic Usage

```rust
use surrealdb_client::{connect_database, DbConfig, StorageEngine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: Option<String>,
    name: String,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a configuration with local file storage
    let config = DbConfig::new(StorageEngine::LocalKv)
        .with_path("./data.db")
        .with_namespace_and_db("test", "users");
    
    // Connect to the database
    let client = connect_database(config).await?;
    
    // Run a query
    let mut response = client.query("CREATE user:john SET name = 'John', email = 'john@example.com'").await?;
    
    // Get a single user
    let user: User = client.get("user", "john").await?
        .expect("User not found");
    
    println!("User: {:?}", user);
    
    Ok(())
}
```

### DAO Pattern

```rust
use surrealdb_client::{connect_database, DbConfig, StorageEngine, Entity, Dao, BaseDao};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    #[serde(flatten)]
    base: BaseEntity,
    username: String,
    email: String,
}

impl Entity for User {
    fn table_name() -> &'static str {
        "users"
    }
    
    fn id(&self) -> Option<String> {
        self.base.id.clone()
    }
    
    fn set_id(&mut self, id: String) {
        self.base.id = Some(id);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a configuration
    let config = DbConfig::new(StorageEngine::LocalKv);
    
    // Connect to the database
    let client = connect_database(config).await?;
    
    // Create a DAO for users
    let user_dao = Dao::<User>::new(client);
    
    // Create a new user
    let mut user = User {
        base: BaseEntity::new(),
        username: "john".to_string(),
        email: "john@example.com".to_string(),
    };
    
    // Save the user
    let user = user_dao.create(&mut user).await?.entity()?;
    
    // Get all users
    let users = user_dao.get_all().await?.entities()?;
    
    Ok(())
}
```

## SurrealKV Showcase

This client includes comprehensive support for [SurrealKV](https://github.com/surrealdb/surrealkv), a versioned, persistent, embedded key-value database. Our implementation offers both high-level SurrealDB client abstractions and direct low-level access to SurrealKV's powerful versioning capabilities.

The following showcase demonstrates the full range of capabilities:

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb_client::{
    connect_database, BaseDao, Dao, Entity, 
    Error, open_surrealkv_store,
    DbConfig, StorageEngine, DatabaseClient
};
use tokio::sync::Mutex;

// Create a type alias for our Result
type Result<T> = std::result::Result<T, Error>;

// Define a simple entity base struct
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaseEntity {
    pub id: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl BaseEntity {
    pub fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: None,
            created_at: Some(now),
            updated_at: Some(now),
        }
    }
}

// Define a versioned document entity for our example
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionedDocument {
    #[serde(flatten)]
    base: BaseEntity,
    title: String,
    content: String,
    version: u32,
    tags: Vec<String>,
}

impl Entity for VersionedDocument {
    fn table_name() -> &'static str {
        "versioned_documents"
    }

    fn id(&self) -> Option<String> {
        self.base.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.base.id = Some(id);
    }
}

impl VersionedDocument {
    fn new(title: impl Into<String>, content: impl Into<String>, tags: Vec<String>) -> Self {
        Self {
            base: BaseEntity::new(),
            title: title.into(),
            content: content.into(),
            version: 1,
            tags,
        }
    }

    fn increment_version(&mut self) {
        self.version += 1;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("SurrealDB Client API with SurrealKV Showcase\n");
    
    // First demonstrate the low-level SurrealKV API
    demonstrate_low_level_kv_api().await?;
    
    // Then showcase the high-level database API
    demonstrate_database_api().await?;
    
    // Finally, demonstrate concurrent access
    demonstrate_concurrent_access().await?;
    
    println!("Demo completed successfully!");
    Ok(())
}

// Demonstrates the low-level SurrealKV API for versioning
async fn demonstrate_low_level_kv_api() -> Result<()> {
    println!("=== Low-level SurrealKV API Demo ===");
    
    // Open a SurrealKV store with versioning enabled
    let store_path = "./.data/demo_kv_store";
    let kv_store = open_surrealkv_store(store_path)?;
    
    // Begin a transaction
    let mut txn = kv_store.begin()?;
    
    // Store a JSON document with versioning
    let document = VersionedDocument::new(
        "My First Document", 
        "This is the first version of my document.",
        vec!["demo".to_string(), "first".to_string()]
    );
    
    // Convert the document to a key and value
    let key = b"doc:001";
    txn.set_json(key, &document)?;
    
    // Commit the transaction
    txn.commit()?;
    
    // Retrieve and update the document in a new transaction
    let mut txn = kv_store.begin()?;
    
    if let Some(mut doc) = txn.get_json::<VersionedDocument>(key)? {
        println!("Retrieved document (v{}): {}", doc.version, doc.title);
        
        // Update the document
        doc.content = "This is the second version of my document.".to_string();
        doc.increment_version();
        
        // Store the updated document
        txn.set_json(key, &doc)?;
        
        // Commit the transaction
        txn.commit()?;
        
        // Begin a new transaction to retrieve both versions
        let mut txn = kv_store.begin()?;
        
        // Get all versions of the document
        let versions = txn.get_all_versions(key)?;
        println!("Number of versions available: {}", versions.len());
        
        for (ts, _) in &versions {
            if let Some(version_doc) = txn.get_json_at_version::<VersionedDocument>(key, *ts)? {
                println!("Document at timestamp {}: v{} - {}", 
                    ts, version_doc.version, version_doc.content);
            }
        }
    }
    
    Ok(())
}

// The main application demo using the high-level API
async fn demonstrate_database_api() -> Result<()> {
    println!("\n=== High-level SurrealDB API Demo ===");
    
    // Create a configuration for SurrealKV storage engine
    let config = DbConfig {
        engine: StorageEngine::SurrealKv,
        path: "./.data/surrealkv_db".to_string(),
        namespace: "demo".to_string(),
        database: "showcase".to_string(),
        run_migrations: true,
        ..Default::default()
    };
    
    // Connect to the database
    let client = connect_database(config).await?;
    
    // Create a DAO for the versioned documents
    let dao = Arc::new(Dao::<VersionedDocument>::new(client.clone()));
    
    // Ensure the table exists
    dao.create_table().success().await?;
    
    // Create a document
    let mut doc = VersionedDocument::new(
        "Important Article", 
        "This article contains important information about SurrealKV.",
        vec!["important".to_string(), "article".to_string(), "surrealkv".to_string()]
    );
    
    // Save the document and get back the created entity
    let created = dao.create(&mut doc).entity().await?;
    println!("Created document: {} (ID: {})", created.title, created.id().unwrap());
    
    // Query for documents with a specific tag using SQL
    let sql = "SELECT * FROM versioned_documents WHERE $tag IN tags";
    let tag_param = serde_json::json!({"tag": "article"});
    
    let articles: Vec<VersionedDocument> = client.query_with_params(sql, tag_param).await?;
    println!("Found {} articles with 'article' tag", articles.len());
    
    // Update document in a transaction
    client.begin_transaction().await?;
    
    // Update the document
    let mut to_update = created.clone();
    to_update.content = "This article has been updated with additional information about SurrealKV.".to_string();
    to_update.tags.push("updated".to_string());
    to_update.increment_version();
    
    // Save the updates
    let updated = dao.update(&to_update).optional_entity().await?;
    
    if let Some(updated) = updated {
        println!("Updated document to version {}", updated.version);
        client.commit_transaction().await?;
    } else {
        println!("Update failed, rolling back transaction");
        client.rollback_transaction().await?;
    }
    
    Ok(())
}
```

### Additional Examples

For more examples of how to use the SurrealDB client with different data models and features, see the [examples](./examples) directory:

- [surrealkv_showcase.rs](./examples/surrealkv_showcase.rs) - Full demonstration of SurrealKV features including versioning and concurrent access
- [graph_relationships.rs](./examples/graph_relationships.rs) - How to model and query graph relationships
- [time_series.rs](./examples/time_series.rs) - Working with time-series data
- [vector_search.rs](./examples/vector_search.rs) - Store and query vector embeddings for AI applications

## Configuration

```rust
// For development
let dev_config = DbConfig::new(StorageEngine::LocalKv)
    .with_path("./dev.db")
    .with_namespace_and_db("dev", "app");

// For production with TiKV
let prod_config = DbConfig::new(StorageEngine::TiKv)
    .with_path("tikv://localhost:2379")
    .with_namespace_and_db("prod", "app")
    .with_credentials("root", "password");

// For tests with in-memory storage
let test_config = DbConfig::new(StorageEngine::Memory)
    .without_migrations()
    .without_metrics();

// For production with SurrealKV
let surrealkv_config = DbConfig::new(StorageEngine::SurrealKv)
    .with_path("/var/lib/app/data.surrealkv")
    .with_namespace_and_db("prod", "app");
```

## Advanced Configuration

### Storage Engine Implementation Details

This library offers multiple storage engine options through a unified interface. Here's an important note about how storage engines are implemented internally:

#### Database Client Implementation

```rust
// Internal DatabaseClient enum from the library
pub enum DatabaseClient {
    /// Local file-based database (for desktop apps, development)
    LocalDb(Surreal<Db>),
    /// Generic database engine (any supported engine)
    Any(Surreal<Any>),
    /// WebSocket connection to remote SurrealDB instance  
    RemoteWs(Surreal<Client>),
    /// SurrealKV embedded key-value store
    SurrealKv(Surreal<Db>),
}
```

Note that both `LocalDb` and `SurrealKv` variants use the same underlying Rust type (`Surreal<Db>`). This is intentional as they both use the same local database driver interface from SurrealDB. The difference is in how the connection string is formatted and how the storage engine is initialized:

- `LocalDb`: Uses `file://` for RocksDB
- `SurrealKv`: Uses `file://` for SurrealKV
- `Any`: Used for TiKV with `tikv://` protocol
- `RemoteWs`: Used for WebSocket with `ws://` or `wss://` protocols

When connecting with `StorageEngine::SurrealKv`, you're not using memory - you're using the SurrealKV filesystem implementation. Similarly, `StorageEngine::TiKV` properly connects to a TiKV cluster.

#### Connection String Formats

Each engine requires specific connection string formats:

- **SurrealKv**:
  ```
  file:///path/to/data.db
  ```

- **LocalKv** (RocksDB):
  ```
  file:///path/to/data.db
  ```

- **TiKV**:
  ```
  tikv://pd1:2379,pd2:2379,pd3:2379
  ```

- **WebSocket**:
  ```
  ws://localhost:8000/rpc
  wss://db.example.com/rpc
  ```

### SurrealKV Tuning Options

For fine-grained control over SurrealKV performance and behavior, use the low-level API with custom options:

```rust
use surrealdb_client::{create_surrealkv_store, SurrealKvStore};
use surrealkv::Options;

// Create custom SurrealKV options
let mut opts = Options::new();
opts.dir = "/path/to/data".into();
opts.enable_versions = true;     // Enable versioning support
opts.max_value_threshold = 8192; // Threshold for large values (bytes)
opts.mem_table_size = 64 * 1024 * 1024; // Size of in-memory table (64MB)
opts.max_files_threshold = 20;   // Maximum number of files before compaction
opts.block_cache_size = 32 * 1024 * 1024; // Size of block cache (32MB)
opts.bloom_filter_bits = 10;     // Bits per key in bloom filter

// Create a store with custom options
let store = create_surrealkv_store(opts)?;

// Begin a transaction
let txn = store.begin()?;

// Use the transaction for key-value operations
// ...

// Alternatively, use the optimized store creator with simplified parameters
use surrealdb_client::create_optimized_surrealkv_store;

// Create a performance-optimized store with 128MB memory limit and versioning enabled
let store = create_optimized_surrealkv_store("/path/to/data", 128, true)?;
```

### TiKV Connection Options

TiKV connection can be configured using the connection string format:

```rust
// Basic TiKV connection
let tikv_config = DbConfig::new(StorageEngine::TiKv)
    .with_path("tikv://pd1:2379,pd2:2379,pd3:2379")
    .with_namespace_and_db("prod", "app");

// TiKV with additional options (using SurrealDB connection string format)
let tikv_config = DbConfig::new(StorageEngine::TiKv)
    .with_path("tikv://pd1:2379,pd2:2379,pd3:2379?timeout=30&retry=5&ttl=600")
    .with_namespace_and_db("prod", "app");

// Or use the dedicated method for TiKV options (recommended)
let tikv_config = DbConfig::new(StorageEngine::TiKv)
    .with_path("tikv://pd1:2379,pd2:2379,pd3:2379")
    .with_tikv_options(30, 5, 600) // timeout_secs, retry_count, ttl_secs
    .with_namespace_and_db("prod", "app");
```

For production deployments, environment variables can also control TiKV connection settings:

```rust
// Set these environment variables before deployment
// TIKV_ENDPOINT=tikv://pd1:2379,pd2:2379,pd3:2379
// TIKV_TIMEOUT=30
// TIKV_RETRY=5
// TIKV_TTL=600

// The library automatically uses these settings when available
let config = DbConfig::new(StorageEngine::TiKv);
```

## License

MIT