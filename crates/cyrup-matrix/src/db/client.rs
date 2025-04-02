use crate::db::config::{DbConfig, StorageEngine};
use crate::db::error::{Error, ErrorContext, Result};
use crate::db::metric;
use crate::db::migration;
use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};
// Path import removed per CLAUDE.md guidelines
use surrealdb::{
    engine::local,
    opt::auth::Root,
    Surreal,
};
use tracing::{debug, info, warn};

/// Unified client for SurrealDB with SurrealKV storage engine
#[derive(Debug)]
pub enum DatabaseClient {
    /// SurrealKV embedded key-value store
    SurrealKv(Surreal<local::Db>),
}

impl Clone for DatabaseClient {
    fn clone(&self) -> Self {
        match self {
            Self::SurrealKv(db) => Self::SurrealKv(db.clone()),
        }
    }
}

impl DatabaseClient {
    /// Extract result from a query using a specific extraction strategy
    async fn extract_result<T>(&self, query: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        // First, try to get results as Vec<T>
        let response = match self {
            DatabaseClient::SurrealKv(db) => db.query(query).await?,
        };

        // Check for query errors
        if let Err(e) = response.check().map_err(Error::SurrealDbError) {
            return Err(e);
        }

        let mut response = match self {
            DatabaseClient::SurrealKv(db) => db.query(query).await?,
        };

        // Try to extract as Vec<T> first
        if let Ok(mut results) = response.take::<Vec<T>>(0_usize) {
            if !results.is_empty() {
                return Ok(results.remove(0));
            }
        }

        // Try to extract as Option<T>
        let mut response = match self {
            DatabaseClient::SurrealKv(db) => db.query(query).await?,
        };

        match response.take::<Option<T>>(0_usize) {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(Error::NotFound(ErrorContext::new("No result found"))),
            Err(_) => {
                // Try once more for a bare value
                let mut response = match self {
                    DatabaseClient::SurrealKv(db) => db.query(query).await?,
                };

                // Convert to a generic result
                let value = response
                    .take::<Option<surrealdb::sql::Value>>(0_usize)
                    .map_err(Error::from)?;

                match value {
                    Some(val) => {
                        // Convert the value to our target type
                        let json_val = serde_json::Value::from(val);
                        serde_json::from_value::<T>(json_val).map_err(Error::Serialization)
                    }
                    None => Err(Error::NotFound(ErrorContext::new("Empty result"))),
                }
            }
        }
    }

    /// Run a SQL query directly
    pub async fn query<T>(&self, query: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let start = std::time::Instant::now();
        let result = self.extract_result::<T>(query).await;
        let duration = start.elapsed();
        metric::record_query_duration(duration);
        result
    }

    /// Extract result from a query with parameters
    async fn extract_result_with_params<T>(
        &self,
        query: &str,
        params: impl Serialize + Clone + Send + 'static,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        // Run the query with parameters
        let response = match self {
            DatabaseClient::SurrealKv(db) => db.query(query).bind(params.clone()).await?,
        };

        // Check for query errors
        if let Err(e) = response.check().map_err(Error::SurrealDbError) {
            return Err(e);
        }

        // Extract result using same strategy as extract_result
        let mut response = match self {
            DatabaseClient::SurrealKv(db) => db.query(query).bind(params.clone()).await?,
        };

        // Try to extract as Vec<T> first
        if let Ok(mut results) = response.take::<Vec<T>>(0_usize) {
            if !results.is_empty() {
                return Ok(results.remove(0));
            }
        }

        // Try to extract as Option<T>
        let mut response = match self {
            DatabaseClient::SurrealKv(db) => db.query(query).bind(params.clone()).await?,
        };

        match response.take::<Option<T>>(0_usize) {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(Error::NotFound(ErrorContext::new("No result found"))),
            Err(_) => {
                // Try once more for a bare value
                let mut response = match self {
                    DatabaseClient::SurrealKv(db) => db.query(query).bind(params.clone()).await?,
                };

                // Convert to a generic result
                let value = response
                    .take::<Option<surrealdb::sql::Value>>(0_usize)
                    .map_err(Error::from)?;

                match value {
                    Some(val) => {
                        // Convert the value to our target type
                        let json_val = serde_json::Value::from(val);
                        serde_json::from_value::<T>(json_val).map_err(Error::Serialization)
                    }
                    None => Err(Error::NotFound(ErrorContext::new("Empty result"))),
                }
            }
        }
    }

    /// Run a SQL query with parameters
    pub async fn query_with_params<T>(
        &self,
        query: &str,
        params: impl Serialize + Clone + Send + 'static,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let start = std::time::Instant::now();
        let result = self.extract_result_with_params::<T>(query, params).await;
        let duration = start.elapsed();
        metric::record_query_duration(duration);
        result
    }

    /// Create a new record
    pub async fn create<T>(
        &self,
        table: &str,
        data: impl Serialize + Clone + Send + 'static,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let start = std::time::Instant::now();

        // Create the record
        let created: Option<T> = match self {
            DatabaseClient::SurrealKv(db) => db.create(table).content(data.clone()).await?,
        };

        // Extract the record (unwrap Option if needed)
        let record =
            created.ok_or_else(|| Error::NotFound(ErrorContext::new("Failed to create record")))?;

        let duration = start.elapsed();
        metric::record_mutation_duration(duration);

        Ok(record)
    }

    /// Select records by query
    pub async fn select<T>(&self, table: &str) -> Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let start = std::time::Instant::now();

        let result = match self {
            DatabaseClient::SurrealKv(db) => db.select(table).await?,
        };

        let duration = start.elapsed();
        metric::record_query_duration(duration);

        Ok(result)
    }

    /// Get a single record by ID
    pub async fn get<T>(&self, table: &str, id: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let start = std::time::Instant::now();

        // Use select method which returns an Option<T> for a single record
        let result: Option<T> = match self {
            DatabaseClient::SurrealKv(db) => db.select((table, id)).await?,
        };

        let duration = start.elapsed();
        metric::record_query_duration(duration);

        Ok(result)
    }

    /// Update a record
    pub async fn update<T>(
        &self,
        table: &str,
        id: &str,
        data: impl Serialize + Clone + Send + 'static,
    ) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let start = std::time::Instant::now();

        // Update returns Option<T> if the record exists
        let result: Option<T> = match self {
            DatabaseClient::SurrealKv(db) => db.update((table, id)).content(data.clone()).await?,
        };

        let duration = start.elapsed();
        metric::record_mutation_duration(duration);

        Ok(result)
    }

    /// Delete a record
    pub async fn delete<T>(&self, table: &str, id: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let start = std::time::Instant::now();

        // Delete returns Option<T> with the deleted record if it existed
        let result: Option<T> = match self {
            DatabaseClient::SurrealKv(db) => db.delete((table, id)).await?,
        };

        let duration = start.elapsed();
        metric::record_mutation_duration(duration);

        Ok(result)
    }

    /// Execute a transaction operation
    async fn execute_transaction(&self, operation: &str) -> Result<()> {
        let query = match operation {
            "begin" => "BEGIN TRANSACTION",
            "commit" => "COMMIT TRANSACTION",
            "rollback" => "ROLLBACK TRANSACTION",
            _ => {
                return Err(Error::other(format!(
                    "Invalid transaction operation: {}",
                    operation
                )))
            }
        };

        let response = match self {
            DatabaseClient::SurrealKv(db) => db.query(query).await?,
        };

        response.check().map_err(Error::SurrealDbError)?;
        Ok(())
    }

    /// Begin a transaction
    pub async fn begin_transaction(&self) -> Result<()> {
        self.execute_transaction("begin").await
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self) -> Result<()> {
        self.execute_transaction("commit").await
    }

    /// Rollback a transaction
    pub async fn rollback_transaction(&self) -> Result<()> {
        self.execute_transaction("rollback").await
    }

    /// Check if the database is healthy
    pub async fn health_check(&self) -> Result<bool> {
        // Use a simple boolean query to check database health
        let response = match self {
            DatabaseClient::SurrealKv(db) => db.query("RETURN true").await?,
        };

        Ok(response.check().is_ok())
    }
}

/// Connect to SurrealDB using the provided configuration
pub async fn connect_database(config: DbConfig) -> Result<DatabaseClient> {
    // Use specific directory for Cyrum database
    let db_path = std::env::var("USER_PREFS_DIR")
        .unwrap_or_else(|_| "./".to_string()) + "/cyrum/cyrum.db";
    
    info!("Connecting to SurrealDB using SurrealKv engine at {}", db_path);
    
    // Create parent directory if needed
    if let Some(parent) = Path::new(&db_path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create database directory: {}", e);
        }
    }

    // Connect to SurrealDB with surrealkv engine using file:// protocol
    let connection_string = format!("file:{}", db_path);
    let db = Surreal::new::<local::Db>(&connection_string).await?;

    if let (Some(ns), Some(db_name)) = (&config.namespace, &config.database) {
        if !ns.is_empty() && !db_name.is_empty() {
            db.use_ns(ns).use_db(db_name).await?;
        }
    }

    // Add authentication if provided
    if let (Some(user), Some(pass)) = (&config.username, &config.password) {
        if !user.is_empty() && !pass.is_empty() {
            db.signin(Root {
                username: user,
                password: pass,
            })
            .await
            .map_err(|e| {
                Error::authentication(ErrorContext::new(format!(
                    "Authentication failed: {}",
                    e
                )))
            })?;
        }
    }

    let client = DatabaseClient::SurrealKv(db);

    // Run migration if configured
    if config.run_migration {
        info!("Running migration...");
        migration::run_migration(&client, migration::get_hardcoded_migration())
            .await
            .map_err(|e| {
                Error::migration(ErrorContext::new(format!(
                    "Failed to run migration: {}",
                    e
                )))
            })?;
    }

    Ok(client)
}

/// Create a new database connection from a configuration

/// Create a new database connection from a configuration
pub async fn new(config: DbConfig) -> Result<DatabaseClient> {
    // Just delegate to connect_database for consistent implementation
    connect_database(config).await
}
