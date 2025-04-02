use crate::error::Error;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::debug;

/// Storage engine for SurrealDB
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageEngine {
    /// SurrealKV storage (for local file-based databases)
    SurrealKv,
}

impl Default for StorageEngine {
    fn default() -> Self {
        Self::SurrealKv
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    /// Storage engine to use
    #[serde(default)]
    pub engine: StorageEngine,

    /// Path to the database file (for local engines) or connection URL (for remote)
    #[serde(default)]
    pub path: Option<String>,

    /// URL for remote connections (WebSocket, TiKV)
    #[serde(default)]
    pub url: Option<String>,

    /// Namespace to use
    #[serde(default = "default_namespace")]
    pub namespace: Option<String>,

    /// Database to use
    #[serde(default = "default_database")]
    pub database: Option<String>,

    /// Username for authentication
    #[serde(default)]
    pub username: Option<String>,

    /// Password for authentication
    #[serde(default)]
    pub password: Option<String>,

    /// Whether to run migration
    #[serde(default)]
    pub run_migration: bool,
}

/// Default constructor for DbConfig
impl Default for DbConfig {
    fn default() -> Self {
        Self {
            engine: StorageEngine::default(),
            path: None,
            url: None,
            namespace: default_namespace(),
            database: default_database(),
            username: None,
            password: None,
            run_migration: true,
        }
    }
}

/// Default namespace
fn default_namespace() -> Option<String> {
    Some("test".to_string())
}

/// Default database
fn default_database() -> Option<String> {
    Some("test".to_string())
}

impl DbConfig {
    /// Create a new database configuration for local development
    pub fn local(path: impl Into<String>) -> Self {
        Self {
            engine: StorageEngine::LocalKv,
            path: Some(path.into()),
            url: None,
            namespace: default_namespace(),
            database: default_database(),
            username: None,
            password: None,
            run_migration: true,
        }
    }

    /// Create a new database configuration with SurrealKV (optimized for local apps)
    pub fn surrealkv(path: impl Into<String>) -> Self {
        Self {
            engine: StorageEngine::SurrealKv,
            path: Some(path.into()),
            url: None,
            namespace: default_namespace(),
            database: default_database(),
            username: None,
            password: None,
            run_migration: true,
        }
    }

    /// Create a new database configuration with TiKV (for clustered deployments)
    pub fn tikv(url: impl Into<String>) -> Self {
        Self {
            engine: StorageEngine::TiKv,
            path: None,
            url: Some(url.into()),
            namespace: default_namespace(),
            database: default_database(),
            username: None,
            password: None,
            run_migration: true,
        }
    }

    /// Create a new database configuration with WebSocket
    pub fn websocket(url: impl Into<String>) -> Self {
        Self {
            engine: StorageEngine::WebSocket,
            path: None,
            url: Some(url.into()),
            namespace: default_namespace(),
            database: default_database(),
            username: None,
            password: None,
            run_migration: true,
        }
    }

    /// Set the namespace
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Set the database
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }

    /// Set the credentials
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Set whether to run migration
    pub fn with_migration(mut self, run_migration: bool) -> Self {
        self.run_migration = run_migration;
        self
    }
}

/// Metrics configuration for the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Whether to collect metric
    pub enabled: bool,

    /// Prefix for metric names
    pub prefix: String,

    /// Whether to include query execution time
    pub query_timing: bool,

    /// Whether to collect table-level metric
    pub table_metric: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefix: "surrealdb".to_string(),
            query_timing: true,
            table_metric: true,
        }
    }
}

impl DbConfig {
    /// Ensures the database directory exists (for file-based storage)
    pub fn ensure_db_dir(&self) -> std::io::Result<()> {
        if self.engine == StorageEngine::LocalKv || self.engine == StorageEngine::SurrealKv {
            if let Some(parent) = Path::new(&self.path.as_ref().unwrap()).parent() {
                debug!("Ensuring database directory exists: {}", parent.display());
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }

    /// Validates the configuration
    pub fn validate(&self) -> Result<()> {
        match self.engine {
            StorageEngine::LocalKv => {
                if self.path.is_none() || self.path.as_ref().unwrap().is_empty() {
                    return Err(Error::validation(
                        "Path is required for LocalKv storage engine",
                    ));
                }
            }
            StorageEngine::TiKv => {
                if self.path.is_none() || !self.path.as_ref().unwrap().starts_with("tikv://") {
                    return Err(Error::validation("TiKV path must start with 'tikv://'"));
                }
            }
            StorageEngine::WebSocket => {
                let path_or_url = self.url.as_ref().or(self.path.as_ref());
                if path_or_url.is_none() {
                    return Err(Error::validation(
                        "URL is required for WebSocket storage engine",
                    ));
                }

                let url_str = path_or_url.unwrap();
                if !url_str.starts_with("ws://") && !url_str.starts_with("wss://") {
                    return Err(Error::validation(
                        "WebSocket URL must start with 'ws://' or 'wss://'",
                    ));
                }
            }
            StorageEngine::SurrealKv => {
                if self.path.is_none() || self.path.as_ref().unwrap().is_empty() {
                    return Err(Error::validation(
                        "Path is required for SurrealKv storage engine",
                    ));
                }

                // Ensure directory exists since SurrealKV requires a valid directory
                if let Some(parent) = Path::new(&self.path.as_ref().unwrap()).parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return Err(Error::validation(format!(
                            "Failed to create directory for SurrealKV: {}",
                            e
                        )));
                    }
                }
            }
            _ => {}
        }

        if self.namespace.is_none() || self.namespace.as_ref().unwrap().is_empty() {
            return Err(Error::validation("Namespace cannot be empty"));
        }

        if self.database.is_none() || self.database.as_ref().unwrap().is_empty() {
            return Err(Error::validation("Database name cannot be empty"));
        }

        Ok(())
    }
}

// Pre-defined configurations for different environments

// Removed unused test_config function

// Removed unused development_config function

// Removed unused production_config function

// Removed unused get_environment_config function
