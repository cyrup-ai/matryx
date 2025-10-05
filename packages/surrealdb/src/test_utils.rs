use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum TestUtilsError {
    #[error("Failed to create tokio runtime: {0}")]
    RuntimeCreation(#[from] std::io::Error),

    #[error("Database connection failed: {0}")]
    DatabaseConnection(#[from] surrealdb::Error),

    #[error("Schema initialization failed: {message}")]
    SchemaInitialization { message: String },

    #[error("Test database cleanup failed: {message}")]
    CleanupFailed { message: String },

    #[error("Configuration error: {field}: {message}")]
    Configuration { field: String, message: String },
}

/// Configuration for test database setup
#[derive(Debug, Clone)]
pub struct TestDatabaseConfig {
    pub backend_url: String,
    pub enable_logging: bool,
    pub timeout_seconds: u64,
    pub max_connections: u32,
}

impl Default for TestDatabaseConfig {
    fn default() -> Self {
        Self {
            backend_url: "surrealkv://test_data/default_test.db".to_string(), // File-based for consistency
            enable_logging: false,             // Quiet tests by default
            timeout_seconds: 30,               // 30 second timeout
            max_connections: 10,               // Connection pool size
        }
    }
}

impl TestDatabaseConfig {
    /// Create config for performance testing with file backend
    pub fn performance() -> Self {
        Self {
            backend_url: "surrealkv://test_data/performance_test.db".to_string(),
            enable_logging: true,
            timeout_seconds: 120, // Longer timeout for performance tests
            max_connections: 50,  // More connections for load testing
        }
    }

    /// Create config for federation testing
    pub fn federation() -> Self {
        Self {
            backend_url: "surrealkv://test_data/federation_test.db".to_string(),
            enable_logging: true, // Enable logging for federation debugging
            timeout_seconds: 60,  // Longer timeout for network operations
            max_connections: 20,  // More connections for federation
        }
    }
}

/// Enhanced test database with isolation and cleanup
pub struct TestDatabase {
    pub db: Surreal<Any>,
    pub database_name: String,
    pub namespace: String,
    runtime: Option<Arc<tokio::runtime::Runtime>>,
}

impl TestDatabase {
    /// Create new isolated test database instance
    pub async fn new() -> Result<Self, TestUtilsError> {
        Self::with_config(TestDatabaseConfig::default()).await
    }

    /// Create test database with custom configuration
    pub async fn with_config(config: TestDatabaseConfig) -> Result<Self, TestUtilsError> {
        // Create unique database name for isolation
        let database_name = format!("test_db_{}", Uuid::new_v4().simple());
        let namespace = format!("test_ns_{}", Uuid::new_v4().simple());

        // Connect to configured backend
        let db = surrealdb::engine::any::connect(&config.backend_url)
            .await
            .map_err(TestUtilsError::DatabaseConnection)?;

        // Use unique namespace and database for complete isolation
        db.use_ns(&namespace)
            .use_db(&database_name)
            .await
            .map_err(TestUtilsError::DatabaseConnection)?;

        // Initialize schema from production migrations
        let migration_sql = include_str!("../migrations/matryx.surql");
        db.query(migration_sql)
            .await
            .map_err(|e| TestUtilsError::SchemaInitialization { message: e.to_string() })?;

        // Properly initialize runtime for Drop cleanup
        let runtime = Some(Arc::new(
            tokio::runtime::Runtime::new().map_err(TestUtilsError::RuntimeCreation)?,
        ));

        Ok(TestDatabase { db, database_name, namespace, runtime })
    }

    /// Seed database with sample Matrix users
    pub async fn seed_test_users(&self, count: usize) -> Result<Vec<String>, TestUtilsError> {
        let mut user_ids = Vec::new();

        for i in 0..count {
            let user_id = format!("@testuser{}:test.localhost", i);

            self.db
                .query(
                    "
                CREATE user_profiles SET
                    user_id = $user_id,
                    display_name = $display_name,
                    avatar_url = $avatar_url,
                    created_at = time::now()
            ",
                )
                .bind(("user_id", user_id.clone()))
                .bind(("display_name", format!("Test User {}", i)))
                .bind(("avatar_url", format!("mxc://test.localhost/avatar{}", i)))
                .await
                .map_err(TestUtilsError::DatabaseConnection)?;

            user_ids.push(user_id);
        }

        Ok(user_ids)
    }

    /// Seed database with sample Matrix rooms
    pub async fn seed_test_rooms(
        &self,
        count: usize,
        creator_user_id: &str,
    ) -> Result<Vec<String>, TestUtilsError> {
        let mut room_ids = Vec::new();

        for i in 0..count {
            let room_id = format!("!testroom{}:test.localhost", i);

            // Create room
            self.db
                .query(
                    "
                CREATE rooms SET
                    room_id = $room_id,
                    room_version = '10',
                    creator = $creator,
                    name = $name,
                    topic = $topic,
                    created_at = time::now()
            ",
                )
                .bind(("room_id", room_id.clone()))
                .bind(("creator", creator_user_id.to_string()))
                .bind(("name", format!("Test Room {}", i)))
                .bind(("topic", format!("Test room {} for integration testing", i)))
                .await
                .map_err(TestUtilsError::DatabaseConnection)?;

            // Add creator as member
            self.db
                .query(
                    "
                CREATE room_membership SET
                    room_id = $room_id,
                    user_id = $user_id,
                    membership = 'join',
                    sender = $user_id,
                    created_at = time::now()
            ",
                )
                .bind(("room_id", room_id.clone()))
                .bind(("user_id", creator_user_id.to_string()))
                .await
                .map_err(TestUtilsError::DatabaseConnection)?;

            room_ids.push(room_id);
        }

        Ok(room_ids)
    }

    /// Seed database with sample devices for testing
    pub async fn seed_test_devices(
        &self,
        user_id: &str,
        count: usize,
    ) -> Result<Vec<String>, TestUtilsError> {
        let mut device_ids = Vec::new();

        for i in 0..count {
            let device_id = format!("TESTDEVICE{}", i);

            self.db
                .query(
                    "
                CREATE device SET
                    device_id = $device_id,
                    user_id = $user_id,
                    display_name = $display_name,
                    last_seen_ip = '127.0.0.1',
                    last_seen_ts = time::now(),
                    created_at = time::now()
            ",
                )
                .bind(("device_id", device_id.clone()))
                .bind(("user_id", user_id.to_string()))
                .bind(("display_name", format!("Test Device {}", i)))
                .await
                .map_err(TestUtilsError::DatabaseConnection)?;

            device_ids.push(device_id);
        }

        Ok(device_ids)
    }

    /// Create sample federation data for server-to-server testing
    pub async fn seed_federation_data(&self) -> Result<(), TestUtilsError> {
        // Create remote server info
        self.db
            .query(
                "
            CREATE server_info SET
                server_name = 'remote.test.localhost',
                server_version = '1.0.0',
                federation_enabled = true,
                signing_keys = {},
                created_at = time::now()
        ",
            )
            .await
            .map_err(TestUtilsError::DatabaseConnection)?;

        // Create federation transaction
        self.db
            .query(
                "
            CREATE federation_transactions SET
                transaction_id = 'test_txn_001',
                origin = 'remote.test.localhost',
                destination = 'test.localhost',
                pdus = [],
                edus = [],
                created_at = time::now()
        ",
            )
            .await
            .map_err(TestUtilsError::DatabaseConnection)?;

        Ok(())
    }

    /// Explicit cleanup method (primary interface)
    pub async fn cleanup(&self) -> Result<(), TestUtilsError> {
        self.db
            .query("REMOVE DATABASE $db")
            .bind(("db", self.database_name.clone()))
            .await
            .map_err(|e| TestUtilsError::CleanupFailed { message: e.to_string() })?;
        Ok(())
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Best-effort cleanup in Drop (fallback only)
        if let Some(rt) = &self.runtime {
            let db = self.db.clone();
            let db_name = self.database_name.clone();

            // Spawn cleanup task - best effort, don't panic on failure
            std::mem::drop(rt.spawn(async move {
                let _ = db.query("REMOVE DATABASE $db").bind(("db", db_name)).await;
            }));
        }
    }
}

/// Create test database with full Matrix schema and isolation
pub async fn create_test_database() -> Result<TestDatabase, TestUtilsError> {
    TestDatabase::new().await
}

/// Create test database with custom configuration
pub async fn create_test_database_with_config(
    config: TestDatabaseConfig,
) -> Result<TestDatabase, TestUtilsError> {
    TestDatabase::with_config(config).await
}

/// Create an async test database connection
pub async fn create_test_db_async() -> Result<Surreal<Any>, TestUtilsError> {
    let db = surrealdb::engine::any::connect("surrealkv://test_data/async_test.db")
        .await
        .map_err(TestUtilsError::DatabaseConnection)?;
    db.use_ns("test")
        .use_db("test")
        .await
        .map_err(TestUtilsError::DatabaseConnection)?;
    Ok(db)
}
