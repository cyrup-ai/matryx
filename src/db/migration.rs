use crate::client::DatabaseClient;
use crate::error::{Error, Result};
use anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
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

/// Create the migration table if it doesn't exist
async fn create_migration_table(client: &DatabaseClient) -> anyhow::Result<()> {
    client
        .query::<()>(
            "
        DEFINE TABLE IF NOT EXISTS migration SCHEMAFULL;
        DEFINE FIELD version ON TABLE migration TYPE string;
        DEFINE FIELD name ON TABLE migration TYPE string;
        DEFINE FIELD executed_at ON TABLE migration TYPE datetime;
        DEFINE FIELD successful ON TABLE migration TYPE bool;
        DEFINE FIELD duration_ms ON TABLE migration TYPE number;
        ",
        )
        .await?;
    Ok(())
}

/// Get a list of already executed migration
async fn get_executed_migration(client: &DatabaseClient) -> anyhow::Result<Vec<String>> {
    let migration: Vec<Migration> = client
        .query("SELECT * FROM migration WHERE successful = true ORDER BY version ASC")
        .await?;

    Ok(migration.into_iter().map(|m| m.version).collect())
}

/// Run a single migration
async fn run_migration(client: &DatabaseClient, version: &str, sql: &str) -> anyhow::Result<()> {
    let start = std::time::Instant::now();

    // Execute the migration in a transaction
    let tx_result = client.begin_transaction().await;
    if let Err(e) = tx_result {
        return Err(anyhow::anyhow!(
            "Failed to begin transaction for migration {}: {}",
            version,
            e
        ));
    }

    let result = client.query::<()>(sql).await;
    let successful = result.is_ok();

    if successful {
        client.commit_transaction().await?;
    } else if let Err(e) = &result {
        warn!("Migration {} failed: {}", version, e);
        client.rollback_transaction().await?;
        return Err(anyhow::anyhow!("Migration {} failed: {}", version, e));
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

    info!(
        "Migration {} completed in {}ms",
        version,
        duration.as_millis()
    );

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
pub async fn run_migration(
    client: &DatabaseClient,
    migration: Vec<(&str, &str)>,
) -> anyhow::Result<()> {
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
        let result = run_migration(client, version, sql).await;
        match result {
            Ok(_) => {
                count += 1;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to run migration {}: {}",
                    version,
                    e
                ));
            }
        }
    }

    info!("Successfully ran {} migration", count);
    Ok(())
}

/// Run migration loaded from a directory
pub async fn run_migration_from_directory(
    client: &DatabaseClient,
    dir: &Path,
) -> anyhow::Result<()> {
    let migration = load_migration_from_directory(dir)?;
    if migration.is_empty() {
        info!("No migration found in directory: {:?}", dir);
        return Ok(());
    }

    create_migration_table(client).await?;

    // Get already executed migration
    let executed = get_executed_migration(client).await?;
    info!("Found {} executed migration", executed.len());

    let mut count = 0;
    for (version, sql) in migration {
        if executed.contains(&version) {
            debug!("Migration {} already executed", version);
            continue;
        }

        info!("Running migration: {}", version);
        let result = run_migration(client, &version, &sql).await;
        match result {
            Ok(_) => {
                count += 1;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to run migration {}: {}",
                    version,
                    e
                ));
            }
        }
    }

    info!("Successfully ran {} migration", count);
    Ok(())
}

/// Run migration with provided content
#[allow(dead_code)]
async fn run_migration_with_content(
    client: &DatabaseClient,
    migration: Vec<(&str, &str)>,
) -> Result<()> {
    // Create migration table if it doesn't exist
    client
        .query::<()>(
            "
        DEFINE TABLE IF NOT EXISTS migration SCHEMAFULL;
        DEFINE FIELD version ON TABLE migration TYPE string;
        DEFINE FIELD name ON TABLE migration TYPE string;
        DEFINE FIELD executed_at ON TABLE migration TYPE datetime;
        DEFINE FIELD successful ON TABLE migration TYPE bool;
        DEFINE FIELD duration_ms ON TABLE migration TYPE number;
        ",
        )
        .await?;

    if migration.is_empty() {
        debug!("No migration available");
        return Ok(());
    }

    // Get already executed migration
    let executed: Vec<Migration> = client
        .query("SELECT * FROM migration ORDER BY version ASC")
        .await?;

    let executed_versions: Vec<String> = executed.iter().map(|m| m.version.clone()).collect();
    info!("Found {} executed migration", executed_versions.len());

    // Find pending migration
    let pending_migration: Vec<(&str, &str)> = migration
        .iter()
        .filter(|(version, _)| !executed_versions.contains(&version.to_string()))
        .cloned()
        .collect();

    if pending_migration.is_empty() {
        info!("No pending migration to run");
        return Ok(());
    }

    info!("Running {} pending migration", pending_migration.len());

    // Run each pending migration
    for (version, sql) in pending_migration {
        info!("Running migration {}", version);
        let start = std::time::Instant::now();

        // Execute the migration in a transaction
        let result = client.begin_transaction().await;
        if let Err(e) = result {
            warn!(
                "Failed to begin transaction for migration {}: {}",
                version, e
            );
            continue;
        }

        let result = client.query::<()>(sql).await;
        let successful = result.is_ok();

        if successful {
            client.commit_transaction().await?;
        } else if let Err(e) = result {
            warn!("Migration {} failed: {}", version, e);
            client.rollback_transaction().await?;
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

        if !successful {
            return Err(Error::migration(format!("Migration {} failed", version)));
        }

        info!(
            "Migration {} completed in {}ms",
            version,
            duration.as_millis()
        );
    }

    info!("All migration completed successfully");
    Ok(())
}

/// Load migration from a directory
/// Each migration should be in a directory like `01-initial`, `02-add-users`, etc.
/// with an `up.surql` file containing the migration SQL
pub fn load_migration_from_directory(dir: &Path) -> io::Result<Vec<(String, String)>> {
    let mut migration = Vec::new();

    if !dir.exists() {
        return Ok(migration);
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
                    migration.push((version.to_string(), sql));
                }
            }
        }
    }

    // Sort migration by version number
    migration.sort_by(|a, b| {
        let a_ver = a.0.parse::<i64>().unwrap_or(0);
        let b_ver = b.0.parse::<i64>().unwrap_or(0);
        a_ver.cmp(&b_ver)
    });

    Ok(migration)
}

/// Get a list of all available hardcoded migration
pub fn get_hardcoded_migration() -> Vec<(&'static str, &'static str)> {
    vec![
        // Matrix-specific schema
        (
            "20250329_000000",
            include_str!("migration/20250329_000000_initial_schema.sql"),
        ),
    ]
}
