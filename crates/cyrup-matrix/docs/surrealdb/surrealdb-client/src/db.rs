use crate::code_document::CodeDocumentDao;
use crate::error::Error;
use crate::migrations;
use anyhow::Result;
use std::path::Path;
use std::sync::LazyLock;
use surrealdb::{engine::local::SurrealKv, Surreal};

static DB: LazyLock<Surreal<SurrealKv>> = LazyLock::new(|| Surreal::init());

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

        // Initialize SurrealKV with versioning support
        *self.db = Surreal::new::<SurrealKv>(db_path).versioned().await?;

        // Use project-specific namespace and database
        self.db.use_ns("project").use_db(project_name).await?;

        // Check if migrations need to be run
        let needs_migration = self.check_migration_needed().await?;

        if needs_migration {
            println!("🔄 Running database migrations...");

            // Run migrations
            migrations::run_migrations(self.db).await?;

            println!("✅ Database migrations completed");
        } else {
            println!("✅ Database 'coder.db' is up to date");
        }

        Ok(())
    }

    /// Check if migrations need to be run
    async fn check_migration_needed(&self) -> Result<bool, Error> {
        // Create migrations table if it doesn't exist
        self.db
            .query(
                "
                DEFINE TABLE IF NOT EXISTS migrations SCHEMAFULL;
                DEFINE FIELD version ON TABLE migrations TYPE string;
                DEFINE FIELD name ON TABLE migrations TYPE string;
                DEFINE FIELD executed_at ON TABLE migrations TYPE datetime;
                ",
            )
            .await?;

        // Get current version
        let mut result = self
            .db
            .query("SELECT version FROM migrations ORDER BY version DESC LIMIT 1")
            .await?;
        let current_version: Option<String> = result.take((0, "version"))?;

        // Compare with latest migration version
        let latest_version = "20240221"; // This should match your latest migration file
        Ok(current_version.map_or(true, |v| {
            v.parse::<i64>().unwrap_or(0) < latest_version.parse::<i64>().unwrap_or(0)
        }))
    }

    /// Get a DAO for code documents
    pub fn code_documents(&self) -> CodeDocumentDao {
        CodeDocumentDao::new(self.db)
    }
}
