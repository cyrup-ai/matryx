# SurrealDB 2.2.1 and surrealdb-migrations 2.2.0 Integration Guide

This document provides guidance on using SurrealDB 2.2.1 with the surrealdb-migrations 2.2.0 library based on the code updates and research conducted.

## SurrealDB 2.2.1 API Changes

The SurrealDB 2.2.1 API has several significant changes from previous versions:

1. **Connection String Format**: 
   - Now uses `file:{path}` format instead of `file://{path}`
   - Example: `connect("file:./data/db")`

2. **Engine Module Structure**: 
   - Changed from `engine::local::Db` to `engine::any::connect`
   - The connection is now created with `connect()` function

3. **Transaction Handling**:
   - Transactions are now handled with cleaner APIs using `begin()`, `commit()`, and `cancel()`

4. **Resource Handling**:
   - Resources can be created using `Resource::from` for type safety

## surrealdb-migrations 2.2.0 Integration

The surrealdb-migrations 2.2.0 library provides a simple way to manage database migrations:

1. **MigrationRunner**:
   ```rust
   // Create a migration runner
   let mut runner = MigrationRunner::new(&db);
   
   // Add migrations
   runner.add_migration_string(id.to_string(), sql.to_string());
   
   // Apply migrations
   runner.up().await?;
   ```

2. **Migration Structure**:
   - Migrations are stored in a YAML file (migrations.yaml) that lists the migration IDs
   - Each migration has its own directory with an up.surql file
   - Example structure:
     ```
     migrations/
       migrations.yaml
       migrations/
         20250329_000000/
           up.surql
         20250330_000001/
           up.surql
     ```

3. **Loading Migrations**:
   - Migrations can be embedded in the binary using `include_dir!`
   - The YAML file defines the migration order
   - Example YAML:
     ```yaml
     migrations:
       - 20250329_000000 # Initial schema
       - 20250330_000001 # Matrix state store
     ```

## Example: Complete Migration Function

```rust
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::fs;
use surrealdb::{engine::any::connect, opt::auth::Root};
use tracing::info;
use surrealdb_migrations::MigrationRunner;
use include_dir::{include_dir, Dir};

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
    
    // Connect to SurrealDB using file: protocol for v2.2.1+ compatibility
    let conn_str = format!("file:{}", path.to_string_lossy());
    let db = connect(&conn_str)
        .with_capacity(10)
        .await
        .context("Failed to connect to database")?;
    
    // Use namespace and database
    db.use_ns("maxtryx").use_db("matrix")
        .await
        .context("Failed to use namespace and database")?;
    
    // Run migrations using surrealdb-migrations v2.2.0+
    info!("Running migrations with surrealdb-migrations v2.2.0...");
    
    // Create a migration runner
    let mut runner = MigrationRunner::new(&db);
    
    // Register migrations from our migrations directory
    let yaml_path = MIGRATIONS_DIR.get_file("migrations.yaml")
        .context("Missing migrations.yaml file")?;
        
    let yaml_content = yaml_path.contents_utf8()
        .context("Failed to read migrations.yaml file")?;
    
    // Parse the migrations.yaml file
    let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_content)
        .context("Failed to parse migrations.yaml")?;
    
    // Get the list of migrations
    let migrations = yaml["migrations"].as_sequence()
        .context("migrations.yaml does not contain a migrations array")?;
    
    // Register each migration
    for migration_id in migrations {
        let id = migration_id.as_str()
            .context("Migration ID must be a string")?;
        
        // Find the migration directory
        let migration_dir = MIGRATIONS_DIR.get_dir(&format!("migrations/{}", id))
            .context(format!("Missing migration directory for {}", id))?;
        
        // Find the up.surql file
        let up_file = migration_dir.get_file("up.surql")
            .context(format!("Missing up.surql file for migration {}", id))?;
        
        let sql = up_file.contents_utf8()
            .context(format!("Failed to read up.surql for migration {}", id))?;
        
        // Register the migration with the runner
        info!("Registering migration: {}", id);
        
        // Add the migration directly using the runner methods
        runner.add_migration_string(id.to_string(), sql.to_string());
    }
    
    // Apply all pending migrations
    info!("Applying migrations...");
    runner.up().await.context("Failed to apply migrations")?;
    
    info!("Matrix StateStore migrations completed successfully");
    Ok(())
}
```

## Database Client Implementation Changes

The DatabaseClient implementation needs significant updates to work with SurrealDB 2.2.1:

1. Update the client struct:
   ```rust
   #[derive(Debug, Clone)]
   pub enum DatabaseClient {
       /// SurrealDB client with SurrealKV storage engine
       SurrealKV(Arc<Surreal<Connection>>),
   }
   ```

2. Update the connection function:
   ```rust
   pub async fn connect_database(config: &DbConfig) -> Result<DatabaseClient> {
       match config.storage_engine {
           StorageEngine::SurrealKV => {
               match config.file_path() {
                   Some(file_path) => {
                       // Connect to database with proper format and connection pool capacity
                       let conn_str = format!("file:{}", file_path);
                       let db = connect(&conn_str)
                           .with_capacity(1000)
                           .await?;
                       
                       // Set namespace and database
                       db.use_ns(config.namespace.clone())
                           .use_db(config.database.clone())
                           .await?;
                       
                       Ok(DatabaseClient::SurrealKV(Arc::new(db)))
                   }
                   None => Err(Error::database(
                       "Database file path not provided".to_string(),
                   )),
               }
           }
       }
   }
   ```

## Further Considerations

1. **Transaction Management**:
   - Use the transaction API for safe database operations
   - Example:
     ```rust
     // Begin transaction
     db.begin().await?;
     
     // Execute queries
     let result = db.query("...").await;
     
     // Commit or cancel
     if result.is_ok() {
         db.commit().await?;
     } else {
         db.cancel().await?;
     }
     ```

2. **Error Handling**:
   - SurrealDB 2.2.1 has better error handling with detailed error types
   - Map database errors to your application's error types

3. **Query Builder**:
   - Use parameter binding for safe queries
   - Example:
     ```rust
     db.query("SELECT * FROM person WHERE name = $name")
       .bind(("name", "John"))
       .await?;
     ```

4. **Session Management**:
   - Each connection can have multiple sessions with different authentication contexts

5. **Live Queries**:
   - SurrealDB 2.2.1 supports live queries for real-time updates
   - Example:
     ```rust
     let live = db.query("LIVE SELECT * FROM person").await?;
     let stream = live.into_stream::<Person>();
     ```

## Migration Best Practices

1. **Version Your Migrations**: Use a clear versioning scheme (date/time-based IDs work well)
2. **Use Transactions**: Wrap migrations in transactions for atomicity
3. **Keep Migrations Idempotent**: Migrations should be safely rerunnable
4. **Test Migrations**: Test migrations on sample data before production
5. **Document Schema Changes**: Include comments in migration files