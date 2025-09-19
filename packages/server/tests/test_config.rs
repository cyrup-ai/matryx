use std::collections::HashMap;
use surrealdb::{Surreal, engine::local::Mem};

/// Test configuration for isolated test database
pub fn test_database_config() -> HashMap<String, String> {
    let mut config = HashMap::new();
    config.insert("url".to_string(), "memory://".to_string());
    config.insert("namespace".to_string(), "test".to_string());
    config.insert("database".to_string(), "matrix_test".to_string());
    config
}

/// Initialize test database with schema
pub async fn init_test_database() -> Result<Surreal<surrealdb::engine::local::Db>, Box<dyn std::error::Error>> {
    let db = Surreal::new::<Mem>(()).await?;
    db.use_ns("test").use_db("matrix_test").await?;
    
    // Load the comprehensive Matrix schema
    let schema = include_str!("../../surrealdb/migrations/matryx.surql");
    db.query(schema).await?;
    
    Ok(db)
}

/// Test environment configuration
pub struct TestConfig {
    pub homeserver_name: String,
    pub server_name: String,
    pub test_timeout_seconds: u64,
    pub max_concurrent_users: u32,
    pub enable_federation_tests: bool,
    pub enable_performance_tests: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            homeserver_name: "test.localhost".to_string(),
            server_name: "test.localhost".to_string(),
            test_timeout_seconds: 30,
            max_concurrent_users: 10,
            enable_federation_tests: true,
            enable_performance_tests: true,
        }
    }
}

impl TestConfig {
    pub fn from_env() -> Self {
        Self {
            homeserver_name: std::env::var("TEST_HOMESERVER_NAME")
                .unwrap_or_else(|_| "test.localhost".to_string()),
            server_name: std::env::var("TEST_SERVER_NAME")
                .unwrap_or_else(|_| "test.localhost".to_string()),
            test_timeout_seconds: std::env::var("TEST_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            max_concurrent_users: std::env::var("TEST_MAX_CONCURRENT_USERS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            enable_federation_tests: std::env::var("TEST_ENABLE_FEDERATION")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_performance_tests: std::env::var("TEST_ENABLE_PERFORMANCE")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        }
    }
}