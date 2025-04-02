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

/// Create the migrations table if it doesn't exist
async fn create_migrations_table(client: &DatabaseClient) -> anyhow::Result<()> {
    client
        .query::<()>(
            "
        DEFINE TABLE IF NOT EXISTS migrations SCHEMAFULL;
        DEFINE FIELD version ON TABLE migrations TYPE string;
        DEFINE FIELD name ON TABLE migrations TYPE string;
        DEFINE FIELD executed_at ON TABLE migrations TYPE datetime;
        DEFINE FIELD successful ON TABLE migrations TYPE bool;
        DEFINE FIELD duration_ms ON TABLE migrations TYPE number;
        ",
        )
        .await?;
    Ok(())
}

/// Get a list of already executed migrations
async fn get_executed_migrations(client: &DatabaseClient) -> anyhow::Result<Vec<String>> {
    let migrations: Vec<Migration> = client
        .query("SELECT * FROM migrations WHERE successful = true ORDER BY version ASC")
        .await?;

    Ok(migrations.into_iter().map(|m| m.version).collect())
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

    client.create::<Migration>("migrations", migration).await?;

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

/// Run migrations on the database using the given client
/// The migrations are provided as a list of (version, sql) tuples
pub async fn run_migrations(
    client: &DatabaseClient,
    migrations: Vec<(&str, &str)>,
) -> anyhow::Result<()> {
    create_migrations_table(client).await?;

    // Get already executed migrations
    let executed = get_executed_migrations(client).await?;
    info!("Found {} executed migrations", executed.len());

    let mut count = 0;
    for (version, sql) in migrations {
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

    info!("Successfully ran {} migrations", count);
    Ok(())
}

/// Run migrations loaded from a directory
pub async fn run_migrations_from_directory(
    client: &DatabaseClient,
    dir: &Path,
) -> anyhow::Result<()> {
    let migrations = load_migrations_from_directory(dir)?;
    if migrations.is_empty() {
        info!("No migrations found in directory: {:?}", dir);
        return Ok(());
    }

    create_migrations_table(client).await?;

    // Get already executed migrations
    let executed = get_executed_migrations(client).await?;
    info!("Found {} executed migrations", executed.len());

    let mut count = 0;
    for (version, sql) in migrations {
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

    info!("Successfully ran {} migrations", count);
    Ok(())
}

/// Run migrations with provided content
#[allow(dead_code)]
async fn run_migrations_with_content(
    client: &DatabaseClient,
    migrations: Vec<(&str, &str)>,
) -> Result<()> {
    // Create migrations table if it doesn't exist
    client
        .query::<()>(
            "
        DEFINE TABLE IF NOT EXISTS migrations SCHEMAFULL;
        DEFINE FIELD version ON TABLE migrations TYPE string;
        DEFINE FIELD name ON TABLE migrations TYPE string;
        DEFINE FIELD executed_at ON TABLE migrations TYPE datetime;
        DEFINE FIELD successful ON TABLE migrations TYPE bool;
        DEFINE FIELD duration_ms ON TABLE migrations TYPE number;
        ",
        )
        .await?;

    if migrations.is_empty() {
        debug!("No migrations available");
        return Ok(());
    }

    // Get already executed migrations
    let executed: Vec<Migration> = client
        .query("SELECT * FROM migrations ORDER BY version ASC")
        .await?;

    let executed_versions: Vec<String> = executed.iter().map(|m| m.version.clone()).collect();
    info!("Found {} executed migrations", executed_versions.len());

    // Find pending migrations
    let pending_migrations: Vec<(&str, &str)> = migrations
        .iter()
        .filter(|(version, _)| !executed_versions.contains(&version.to_string()))
        .cloned()
        .collect();

    if pending_migrations.is_empty() {
        info!("No pending migrations to run");
        return Ok(());
    }

    info!("Running {} pending migrations", pending_migrations.len());

    // Run each pending migration
    for (version, sql) in pending_migrations {
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

        client.create::<Migration>("migrations", migration).await?;

        if !successful {
            return Err(Error::migration(format!("Migration {} failed", version)));
        }

        info!(
            "Migration {} completed in {}ms",
            version,
            duration.as_millis()
        );
    }

    info!("All migrations completed successfully");
    Ok(())
}

/// Load migrations from a directory
/// Each migration should be in a directory like `01-initial`, `02-add-users`, etc.
/// with an `up.surql` file containing the migration SQL
pub fn load_migrations_from_directory(dir: &Path) -> io::Result<Vec<(String, String)>> {
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

    // Sort migrations by version number
    migrations.sort_by(|a, b| {
        let a_ver = a.0.parse::<i64>().unwrap_or(0);
        let b_ver = b.0.parse::<i64>().unwrap_or(0);
        a_ver.cmp(&b_ver)
    });

    Ok(migrations)
}

/// Get a list of all available hardcoded migrations
pub fn get_hardcoded_migrations() -> Vec<(&'static str, &'static str)> {
    vec![
        // Initial schema
        (
            "20230101000000",
            r#"
            -- Initial schema setup
            DEFINE TABLE users SCHEMAFULL;
            DEFINE FIELD name ON TABLE users TYPE string;
            DEFINE FIELD email ON TABLE users TYPE string;
            DEFINE FIELD created_at ON TABLE users TYPE datetime;
            DEFINE INDEX users_email ON TABLE users COLUMNS email UNIQUE;
            
            DEFINE TABLE settings SCHEMAFULL;
            DEFINE FIELD key ON TABLE settings TYPE string;
            DEFINE FIELD value ON TABLE settings TYPE string;
            DEFINE FIELD created_at ON TABLE settings TYPE datetime;
            DEFINE FIELD updated_at ON TABLE settings TYPE datetime;
            DEFINE INDEX settings_key ON TABLE settings COLUMNS key UNIQUE;
        "#,
        ),
        // Add more migrations here
        (
            "20230201000000",
            r#"
            -- Add sessions table
            DEFINE TABLE sessions SCHEMAFULL;
            DEFINE FIELD user ON TABLE sessions TYPE record(users);
            DEFINE FIELD token ON TABLE sessions TYPE string;
            DEFINE FIELD expires_at ON TABLE sessions TYPE datetime;
            DEFINE FIELD created_at ON TABLE sessions TYPE datetime;
            DEFINE INDEX sessions_token ON TABLE sessions COLUMNS token UNIQUE;
        "#,
        ),
        // Example of a data migration
        (
            "20230301000000",
            r#"
            -- Add default settings
            INSERT INTO settings (id, key, value, created_at, updated_at)
            VALUES
                ('settings:theme', 'theme', 'light', time::now(), time::now()),
                ('settings:language', 'language', 'en', time::now(), time::now())
            UNLESS $value = NULL;
        "#,
        ),
    ]
}
