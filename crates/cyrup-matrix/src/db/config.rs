use crate::db::error::Result;
use std::path::Path;
use tracing::debug;

/// Storage engine for SurrealDB
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageEngine {
    /// SurrealKV storage (for local file-based databases)
    SurrealKv,
}

/// Database configuration with hardcoded values
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Storage engine to use - only SurrealKv supported
    pub engine: StorageEngine,
    
    /// Path to the database file
    pub path: String,
    
    /// Namespace to use
    pub namespace: String,
    
    /// Database to use
    pub database: String,
    
    /// Whether pending migrations should be checked and applied
    pub check_migrations: bool,
}

impl Default for DbConfig {
    fn default() -> Self {
        // Use nix user directory for database
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("./"))
            .join("cyrum")
            .join("cyrum.db")
            .to_string_lossy()
            .to_string();
            
        Self {
            engine: StorageEngine::SurrealKv,
            path: db_path,
            namespace: "cyrum".to_string(),
            database: "matrix".to_string(),
            check_migrations: true,
        }
    }
}

impl DbConfig {
    /// Ensures the database directory exists
    pub fn ensure_db_dir(&self) -> std::io::Result<()> {
        if let Some(parent) = Path::new(&self.path).parent() {
            debug!("Ensuring database directory exists: {}", parent.display());
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Validates that required paths exist
    pub fn validate(&self) -> Result<()> {
        // Just ensure directory exists - all values are hardcoded
        if let Some(parent) = Path::new(&self.path).parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| crate::db::error::Error::other(format!(
                        "Failed to create database directory: {}", e
                    )))?;
            }
        }
        
        Ok(())
    }
}
