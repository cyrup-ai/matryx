use crate::db::client::DatabaseClient;
use crate::db::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use tokio::sync::mpsc::{self, Receiver};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// A migration record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Migration {
    id: Option<String>,
    version: String,
    name: String,
    executed_at: DateTime<Utc>,
    successful: bool,
    duration_ms: u64,
}

// Domain-specific result types
pub struct MigrationResult {
    rx: Receiver<Result<()>>,
    _handle: JoinHandle<()>,
}

pub struct FileLoadResult {
    rx: Receiver<io::Result<Vec<(String, String)>>>,
    _handle: JoinHandle<()>,
}

impl MigrationResult {
    fn new(rx: Receiver<Result<()>>, handle: JoinHandle<()>) -> Self {
        Self { rx, _handle: handle }
    }

    pub async fn get(mut self) -> Result<()> {
        self.rx
            .recv()
            .await
            .unwrap_or_else(|| Err(Error::other("Channel closed unexpectedly")))
    }
}

impl FileLoadResult {
    fn new(rx: Receiver<io::Result<Vec<(String, String)>>>, handle: JoinHandle<()>) -> Self {
        Self { rx, _handle: handle }
    }

    pub async fn get(mut self) -> io::Result<Vec<(String, String)>> {
        self.rx.recv().await.unwrap_or_else(|| {
            Err(io::Error::new(io::ErrorKind::Other, "Channel closed unexpectedly"))
        })
    }
}

// Private implementation functions (still async but not exposed externally)
async fn create_migration_table(client: &DatabaseClient) -> Result<()> {
    let sql = "
        DEFINE TABLE IF NOT EXISTS migration SCHEMAFULL;
        DEFINE FIELD version ON TABLE migration TYPE string;
        DEFINE FIELD name ON TABLE migration TYPE string;
        DEFINE FIELD executed_at ON TABLE migration TYPE datetime;
        DEFINE FIELD successful ON TABLE migration TYPE bool;
        DEFINE FIELD duration_ms ON TABLE migration TYPE number;
    ";

    client.query::<()>(sql).await?;
    Ok(())
}

async fn get_executed_migration(client: &DatabaseClient) -> Result<Vec<String>> {
    let sql = "SELECT * FROM migration WHERE successful = true ORDER BY version ASC";
    let migration: Vec<Migration> = client.query(sql).await?;
    Ok(migration.into_iter().map(|m| m.version).collect())
}

async fn run_single_migration(client: &DatabaseClient, version: &str, sql: &str) -> Result<()> {
    let start = std::time::Instant::now();

    // Execute the migration in a transaction
    let tx_manager = client.transaction();
    let tx = tx_manager.begin().await?;

    let result = client.query::<()>(sql).await;
    let successful = result.is_ok();

    if successful {
        tx.commit().await?;
    } else if let Err(e) = &result {
        warn!("Migration {} failed: {}", version, e);
        tx.cancel().await?;
        return Err(Error::database(format!("Migration {} failed: {}", version, e)));
    }

    let duration = start.elapsed();

    // Record the migration
    let migration = Migration {
        id: None,
        version: version.to_string(),
        name: format!("Migration {}", version),
        executed_at: Utc::now(),
        successful,
        duration_ms: duration.as_millis() as u64,
    };

    client.create::<Migration>("migration", migration).await?;

    info!("Migration {} completed in {}ms", version, duration.as_millis());

    Ok(())
}

/// A migration file containing SQL statements
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MigrationFile {
    /// Migration version/timestamp
    pub version: String,
    /// Name of the migration
    pub name: String,
    /// SQL content of the migration
    pub sql: String,
}

/// Run migration on the database using the given client
/// The migration are provided as a list of (version, sql) tuples
pub fn run_migration(
    client: &DatabaseClient,
    migration: Vec<(&'static str, &'static str)>,
) -> MigrationResult {
    let (tx, rx) = mpsc::channel(1);
    let client = client.clone();
    let migration = migration.clone();

    let handle = tokio::spawn(async move {
        // Private async implementation
        async fn run_migration_internal(
            client: &DatabaseClient,
            migration: Vec<(&str, &str)>,
        ) -> Result<()> {
            create_migration_table(client).await?;

            // Get already executed migration
            let executed = get_executed_migration(client).await?;
            info!("Found {} executed migration", executed.len());

            let mut count = 0;
            for (version, sql) in migration {
                if executed.contains(&version.to_string()) {
                    debug!("Migration {} already executed", version);
                    continue;
                }

                info!("Running migration: {}", version);
                let result = run_single_migration(client, version, sql).await;
                match result {
                    Ok(_) => {
                        count += 1;
                    },
                    Err(e) => {
                        return Err(Error::database(format!(
                            "Failed to run migration {}: {}",
                            version, e
                        )));
                    },
                }
            }

            info!("Successfully ran {} migration", count);
            Ok(())
        }

        let result = run_migration_internal(&client, migration).await;
        let _ = tx.send(result).await;
    });

    MigrationResult::new(rx, handle)
}

/// Run migration loaded from a directory
pub fn run_migration_from_directory(client: &DatabaseClient, dir: &Path) -> MigrationResult {
    let (tx, rx) = mpsc::channel(1);
    let client = client.clone();
    let dir = dir.to_path_buf();

    let handle = tokio::spawn(async move {
        // Private async implementation
        async fn run_migration_from_directory_internal(
            client: &DatabaseClient,
            dir: &Path,
        ) -> Result<()> {
            // First, load migrations from directory using the synchronous helper
            let migrations = match fs::read_dir(dir) {
                Ok(entries) => {
                    let mut migrations = Vec::new();

                    for entry in entries {
                        if let Ok(entry) = entry {
                            let path = entry.path();

                            if path.is_file() && path.extension().map_or(false, |ext| ext == "sql")
                            {
                                if let Some(file_name) = path.file_stem() {
                                    let file_name = file_name.to_string_lossy();
                                    if let Some(version) = file_name.split('_').next() {
                                        if version.parse::<i64>().is_ok() {
                                            if let Ok(sql) = fs::read_to_string(&path) {
                                                migrations.push((version.to_string(), sql));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Sort migration by version number
                    migrations.sort_by(|a, b| {
                        let a_ver = a.0.parse::<i64>().unwrap_or(0);
                        let b_ver = b.0.parse::<i64>().unwrap_or(0);
                        a_ver.cmp(&b_ver)
                    });

                    migrations
                },
                Err(_) => Vec::new(),
            };

            if migrations.is_empty() {
                info!("No migration found in directory: {:?}", dir);
                return Ok(());
            }

            create_migration_table(client).await?;

            // Get already executed migration
            let executed = get_executed_migration(client).await?;
            info!("Found {} executed migration", executed.len());

            let mut count = 0;
            for (version, sql) in migrations {
                if executed.contains(&version) {
                    debug!("Migration {} already executed", version);
                    continue;
                }

                info!("Running migration: {}", version);
                let result = run_single_migration(client, &version, &sql).await;
                match result {
                    Ok(_) => {
                        count += 1;
                    },
                    Err(e) => {
                        return Err(Error::database(format!(
                            "Failed to run migration {}: {}",
                            version, e
                        )));
                    },
                }
            }

            info!("Successfully ran {} migration", count);
            Ok(())
        }

        let result = run_migration_from_directory_internal(&client, &dir).await;
        let _ = tx.send(result).await;
    });

    MigrationResult::new(rx, handle)
}

/// Load migration from a directory
/// Each migration should be in a directory like `01-initial`, `02-add-users`, etc.
/// with an `up.surql` file containing the migration SQL
pub fn load_migration_from_directory(dir: &Path) -> io::Result<Vec<(String, String)>> {
    // This is now a synchronous function that directly reads from the filesystem
    let mut migrations = Vec::new();

    if !dir.exists() {
        return Ok(migrations);
    }

    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "sql") {
            let file_name = path.file_stem().unwrap().to_string_lossy();
            if let Some(version) = file_name.split('_').next() {
                if version.parse::<i64>().is_ok() {
                    let sql = fs::read_to_string(&path)?;
                    migrations.push((version.to_string(), sql));
                }
            }
        }
    }

    // Sort migration by version number
    migrations.sort_by(|a, b| {
        let a_ver = a.0.parse::<i64>().unwrap_or(0);
        let b_ver = b.0.parse::<i64>().unwrap_or(0);
        a_ver.cmp(&b_ver)
    });

    Ok(migrations)
}

/// Get a list of all available hardcoded migration
pub fn get_hardcoded_migration() -> Vec<(&'static str, &'static str)> {
    vec![
        // Matrix-specific schema
        ("20250329_000000", include_str!("migration/20250329_000000_initial_schema.sql")),
        // Matrix StateStore schema
        ("20250330_000001", include_str!("migration/20250330_000001_matrix_state_store.sql")),
    ]
}

/// Get a list of only the Matrix StateStore migrations
pub fn get_matrix_state_store_migrations() -> Vec<(&'static str, &'static str)> {
    vec![("20250330_000001", include_str!("migration/20250330_000001_matrix_state_store.sql"))]
}
