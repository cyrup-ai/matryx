use crate::db::error::{Error, Result};
use std::path::Path;

/// SurrealDB storage engine options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageEngine {
    /// SurrealKV local file-based storage engine
    SurrealKV,
}

/// Database configuration
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Storage engine to use
    pub storage_engine: StorageEngine,
    /// Database file path for file-based engines
    pub file_path: Option<String>,
    /// Namespace to use
    pub namespace: String,
    /// Database name to use
    pub database: String,
    /// Whether to run migrations
    pub check_migrations: bool,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            storage_engine: StorageEngine::SurrealKV,
            file_path: Some("data/db".to_string()),
            namespace: "cyrup".to_string(),
            database: "main".to_string(),
            check_migrations: true,
        }
    }
}

impl DbConfig {
    /// Create a new database configuration
    pub fn new(
        storage_engine: StorageEngine,
        file_path: Option<String>,
        namespace: String,
        database: String,
        check_migrations: bool,
    ) -> Self {
        Self {
            storage_engine,
            file_path,
            namespace,
            database,
            check_migrations,
        }
    }

    /// Get the file path
    pub fn file_path(&self) -> Option<String> {
        self.file_path.clone()
    }

    /// Ensure the database directory exists
    pub fn ensure_db_dir(&self) -> Result<()> {
        if let Some(path) = &self.file_path {
            let db_path = Path::new(path);
            if let Some(parent) = db_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        Error::other(format!(
                            "Failed to create database directory {}: {}",
                            parent.display(),
                            e
                        ))
                    })?;
                }
            }
        }

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Check namespace and database
        if self.namespace.is_empty() {
            return Err(Error::other("Namespace cannot be empty"));
        }

        if self.database.is_empty() {
            return Err(Error::other("Database name cannot be empty"));
        }

        // For SurrealKV, file_path is required
        match self.storage_engine {
            StorageEngine::SurrealKV => {
                if self.file_path.is_none() {
                    return Err(Error::other("File path is required for SurrealKV storage engine"));
                }
            },
        }

        Ok(())
    }
}
