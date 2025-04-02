// Remove imports for now as they're not needed
use anyhow::Result;
use std::path::Path;
use std::sync::LazyLock;
use surrealdb::{engine::local::Db, Surreal};
use crate::db::error::Error;

static DB: LazyLock<Surreal<Db>> = LazyLock::new(|| Surreal::init());

/// Database Access Object for managing database connections and operations
#[derive(Clone)]
pub struct Dao {
    db: &'static Surreal<Db>,
}

impl Dao {
    /// Create a new DAO instance
    pub fn new() -> Self {
        Self { db: &DB }
    }

    /// Initialize a project-specific database
    pub async fn initialize_project(&self, project_path: impl AsRef<Path>) -> Result<(), Error> {
        // Create .coder directory in project root
        let db_dir = project_path.as_ref().join(".coder");
        std::fs::create_dir_all(&db_dir)?;

        // Use project name as database name
        let project_name = project_path
            .as_ref()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default");

        // Database file path
        let db_path = db_dir.join("coder.db");
        println!("Initializing project database at: {}", db_path.display());

        // Initialize with Db engine and versioning support
        let connection_string = format!("file:{}", db_path.to_string_lossy());
        *self.db = Surreal::new::<surrealdb::engine::local::Db>(&connection_string).await?;

        // Use project-specific namespace and database
        self.db.use_ns("project").use_db(project_name).await?;

        // Check if migration need to be run
        let needs_migration = self.check_migration_needed().await?;

        if needs_migration {
            println!("🔄 Running database migration...");

            // Run migration
            crate::db::migration::run_migration(self.db).await?;

            println!("✅ Database migration completed");
        } else {
            println!("✅ Database 'coder.db' is up to date");
        }

        Ok(())
    }

    /// Check if migration need to be run
    async fn check_migration_needed(&self) -> Result<bool, Error> {
        // Create migration table if it doesn't exist
        self.db
            .query(
                "
                DEFINE TABLE IF NOT EXISTS migration SCHEMAFULL;
                DEFINE FIELD version ON TABLE migration TYPE string;
                DEFINE FIELD name ON TABLE migration TYPE string;
                DEFINE FIELD executed_at ON TABLE migration TYPE datetime;
                ",
            )
            .await?;

        // Get current version
        let mut result = self
            .db
            .query("SELECT version FROM migration ORDER BY version DESC LIMIT 1")
            .await?;
        let current_version: Option<String> = result.take((0, "version"))?;

        // Compare with latest migration version
        let latest_version = "20250329_000000"; // This matches our latest Matrix schema migration
        Ok(current_version.map_or(true, |v| {
            v.parse::<i64>().unwrap_or(0) < latest_version.parse::<i64>().unwrap_or(0)
        }))
    }

    /// Get a DAO for code documents
    
    /// Get a DAO for room membership
    pub fn room_membership(&self) -> crate::db::RoomMembershipDao {
        crate::db::RoomMembershipDao::new(crate::db::client::DatabaseClient::SurrealKv((*self.db).clone()))
    }
    
    /// Get a DAO for messages
    pub fn messages(&self) -> crate::db::MessageDao {
        crate::db::MessageDao::new(crate::db::client::DatabaseClient::SurrealKv((*self.db).clone()))
    }
}
