use crate::db::error::{Error, Result};
use std::path::Path;
use std::sync::LazyLock;
use surrealdb::Surreal;
use crate::db::client::{connect_database, DatabaseClient};
use crate::db::config::DbConfig;
use crate::db::dao::account_data::AccountDataDao;
use crate::db::dao::api_cache::ApiCacheDao;
use crate::db::dao::custom::CustomDao;
use crate::db::dao::key_value::KeyValueDao;
use crate::db::dao::media_upload::MediaUploadDao;
use crate::db::dao::message::MessageDao;
use crate::db::dao::presence::PresenceDao;
use crate::db::dao::receipt::ReceiptDao;
use crate::db::dao::request_dependency::RequestDependencyDao;
use crate::db::dao::room_membership::RoomMembershipDao;
use crate::db::dao::room_state::RoomStateDao;
use crate::db::dao::send_queue::SendQueueDao;

// Use type alias to avoid exposing the private Connect type
type SurrealDbConnection = surrealdb::engine::any::Any;

static DB: LazyLock<Surreal<SurrealDbConnection>> = LazyLock::new(|| Surreal::init());

/// Database Access Object for managing database connections and operations
#[derive(Clone)]
pub struct Dao {
    db: &'static Surreal<SurrealDbConnection>,
}

impl Dao {
    /// Create a new DAO instance
    pub fn new() -> Self {
        Self { db: &DB }
    }

    /// Initialize a project-specific database
    pub async fn initialize_project(&self, project_path: impl AsRef<Path>) -> Result<()> {
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

        // Initialize with SurrealDbConnection engine and versioning support using file:// protocol for 2.2.1+
        let connection_string = format!("file://{}", db_path.to_string_lossy());
        *self.db = surrealdb::engine::any::connect(&connection_string).await?;

        // Use project-specific namespace and database
        self.db.use_ns("project").use_db(project_name).await?;

        // Check if migration need to be run
        let needs_migration = self.check_migration_needed().await?;

        if needs_migration {
            println!("🔄 Running database migration...");

            // Run migration using hardcoded migrations
            crate::db::migration::run_migration(
                &crate::db::client::DatabaseClient::SurrealKV(std::sync::Arc::new(
                    (*self.db).clone(),
                )),
                crate::db::migration::get_hardcoded_migration(),
            )
            .await?;

            println!("✅ Database migration completed");
        } else {
            println!("✅ Database 'coder.db' is up to date");
        }

        Ok(())
    }

    /// Check if migration need to be run
    async fn check_migration_needed(&self) -> Result<bool> {
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

    /// Get a Room Membership DAO for this database
    pub fn room_membership_dao(&self) -> crate::db::dao::room_membership_dao::RoomMembershipDao {
        crate::db::dao::room_membership_dao::RoomMembershipDao::new(
            crate::db::client::DatabaseClient::SurrealKV(std::sync::Arc::new((*self.db).clone())),
        )
    }

    /// Get a Message DAO for this database
    pub fn message_dao(&self) -> crate::db::dao::message_dao::MessageDao {
        crate::db::dao::message_dao::MessageDao::new(crate::db::client::DatabaseClient::SurrealKV(
            std::sync::Arc::new((*self.db).clone()),
        ))
    }
}

/// Initialize a database with the given configuration
pub async fn initialize_database(config: &DbConfig) -> Result<DatabaseClient> {
    // Connect to the database
    let client = connect_database(config).await?;
    
    // Run migrations if needed
    run_migrations(&client).await?;
    
    Ok(client)
}

/// Run database migrations
pub async fn run_migrations(client: &DatabaseClient) -> Result<()> {
    // This implementation is simplified for now
    // In a real-world implementation, we would use surrealdb-migrations
    // or a similar library to handle database schema migrations

    // Just create required tables for now
    create_tables(client).await?;

    Ok(())
}

/// Create required tables
async fn create_tables(client: &DatabaseClient) -> Result<()> {
    // Create tables for each entity type
    create_account_data_table(client).await?;
    create_api_cache_table(client).await?;
    create_custom_value_table(client).await?;
    create_key_value_table(client).await?;
    create_media_upload_table(client).await?;
    create_message_table(client).await?;
    create_presence_table(client).await?;
    create_receipt_table(client).await?;
    create_request_dependency_table(client).await?;
    create_room_membership_table(client).await?;
    create_room_state_table(client).await?;
    create_send_queue_table(client).await?;

    Ok(())
}

/// Create DAOs for all entity types
pub fn create_daos(client: DatabaseClient) -> (
    AccountDataDao,
    ApiCacheDao,
    CustomDao,
    KeyValueDao,
    MediaUploadDao,
    MessageDao,
    PresenceDao,
    ReceiptDao,
    RequestDependencyDao,
    RoomMembershipDao,
    RoomStateDao,
    SendQueueDao,
) {
    (
        AccountDataDao::new(client.clone()),
        ApiCacheDao::new(client.clone()),
        CustomDao::new(client.clone()),
        KeyValueDao::new(client.clone()),
        MediaUploadDao::new(client.clone()),
        MessageDao::new(client.clone()),
        PresenceDao::new(client.clone()),
        ReceiptDao::new(client.clone()),
        RequestDependencyDao::new(client.clone()),
        RoomMembershipDao::new(client.clone()),
        RoomStateDao::new(client.clone()),
        SendQueueDao::new(client.clone()),
    )
}

// Helper functions to create tables
async fn create_account_data_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_api_cache_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_custom_value_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_key_value_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_media_upload_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_message_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_presence_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_receipt_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_request_dependency_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_room_membership_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_room_state_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}

async fn create_send_queue_table(_client: &DatabaseClient) -> Result<()> {
    // TODO: Implement table creation
    Ok(())
}
