use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::fs;
use std::path::PathBuf;
use surrealdb::engine::any::connect;
use surrealdb_migrations::MigrationRunner;
use tracing::info;

// Include migrations directory
static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// Run Matrix StateStore migrations
pub async fn migrate() -> Result<()> {
    info!("Running Matrix StateStore migrations");

    // Default path for database
    let path = PathBuf::from("./data/matrix.db");

    // Create the database directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create database directory")?;
    }

    // Connect to SurrealDB using file: protocol for v2.2.1+ compatibility
    let conn_str = format!("file:{}", path.to_string_lossy());
    let db = connect(&conn_str)
        .with_capacity(10)
        .await
        .context("Failed to connect to database")?;

    // Use namespace and database
    db.use_ns("maxtryx")
        .use_db("matrix")
        .await
        .context("Failed to use namespace and database")?;

    // Run migrations using surrealdb-migrations v2.2.0+
    info!("Running migrations with surrealdb-migrations v2.2.0...");

    // Create a migration runner
    let mut _runner = MigrationRunner::new(&db);

    // Register migrations from our migrations directory
    let yaml_path = MIGRATIONS_DIR
        .get_file("migrations.yaml")
        .context("Missing migrations.yaml file")?;

    let yaml_content = yaml_path.contents_utf8().context("Failed to read migrations.yaml file")?;

    // Parse the migrations.yaml file
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(yaml_content).context("Failed to parse migrations.yaml")?;

    // Get the list of migrations
    let migrations = yaml["migrations"]
        .as_sequence()
        .context("migrations.yaml does not contain a migrations array")?;

    // Register each migration
    for migration_id in migrations {
        let id = migration_id.as_str().context("Migration ID must be a string")?;

        // Find the migration directory
        let migration_dir = MIGRATIONS_DIR
            .get_dir(&format!("migrations/{}", id))
            .context(format!("Missing migration directory for {}", id))?;

        // Find the up.surql file
        let up_file = migration_dir
            .get_file("up.surql")
            .context(format!("Missing up.surql file for migration {}", id))?;

        let sql = up_file
            .contents_utf8()
            .context(format!("Failed to read up.surql for migration {}", id))?;

        // Register the migration with the runner
        info!("Registering migration: {}", id);

        // Execute the migration directly since surrealdb-migrations 2.2.0 doesn't have add_migration_string
        db.query(sql)
            .await
            .context(format!("Failed to execute migration SQL for {}", id))?;
    }

    // Migrations are already applied directly
    info!("All migrations applied successfully");

    info!("Matrix StateStore migrations completed successfully");
    Ok(())
}
