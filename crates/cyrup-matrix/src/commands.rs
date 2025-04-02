use anyhow::{Context, Result};
use std::path::PathBuf;
use std::fs;
use surrealdb::{
    engine::local::Db,
    opt::auth::Root,
    Surreal,
};
use tracing::info;
use surrealdb_migrations::MigrationRunner;
use include_dir::{include_dir, Dir};
// This is a temporary fix - we may need to update the dependency later

// Include migrations directory
static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// Run Matrix StateStore migrations
pub async fn migrate() -> Result<()> {
    info!("Running Matrix StateStore migrations");
    
    // Default path for database
    let path = PathBuf::from("./data/matrix.db");
    
    // Create the database directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create database directory")?;
    }
    
    // Connect to SurrealDB with Db engine (recommended for local apps)
    let connection_string = format!("file:{}", path.to_string_lossy());
    let db = Surreal::new::<Db>(&connection_string)
        .await
        .context("Failed to connect to database")?;
    
    // Sign in as root (default credentials for development)
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await
    .context("Failed to sign in to database")?;
    
    // Use namespace and database
    db.use_ns("cyrum").use_db("matrix")
        .await
        .context("Failed to use namespace and database")?;
    
    // Run migrations using surrealdb-migrations
    info!("Running migrations with surrealdb-migrations...");
    
    // Create a migration runner with the embedded migrations
    let mut runner = MigrationRunner::new(&db);
    runner.set_migration_dir(&MIGRATIONS_DIR);
    runner.up()
        .await
        .context("Failed to apply migrations")?;
    
    info!("Matrix StateStore migrations completed successfully");
    Ok(())
}