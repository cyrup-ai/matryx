use crate::db::error::Result;
use std::path::Path;
use std::sync::LazyLock;
use surrealdb::Surreal;
use crate::db::client::{connect_database, DatabaseClient};
use crate::db::config::DbConfig;
// Import all DAOs from the dao module, which re-exports them
use crate::db::dao::{
    AccountDataDao, ApiCacheDao, CustomDao, KeyValueDao, MediaUploadDao, MessageDao, PresenceDao,
    ReceiptDao, RequestDependencyDao, RoomMembershipDao, RoomStateDao, SendQueueDao,
};

// Use type alias to avoid exposing the private Connect type
type SurrealDbConnection = surrealdb::engine::any::Any;

static DB: LazyLock<Surreal<SurrealDbConnection>> = LazyLock::new(Surreal::init);

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
    pub async fn initialize_project(&mut self, project_path: impl AsRef<Path>) -> Result<()> {
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
        self.db.connect(&connection_string).await?;

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
            .get()
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
        let mut response = self.db
            .query(
                "
                DEFINE TABLE IF NOT EXISTS migration SCHEMAFULL;
                DEFINE FIELD version ON TABLE migration TYPE string;
                DEFINE FIELD name ON TABLE migration TYPE string;
                DEFINE FIELD executed_at ON TABLE migration TYPE datetime;
                "
            )
            .await?;
        // For CREATE/DEFINE queries that return (), we don't need to take any result
        let _ = response;

        // Get current version
        let mut result = self
            .db
            .query("SELECT version FROM migration ORDER BY version DESC LIMIT 1")
            .await?;
        let current_version: Option<String> = result.take("result")?;

        // Compare with latest migration version
        let latest_version = "20250329_000000"; // This matches our latest Matrix schema migration
        Ok(current_version.map_or(true, |v| {
            v.parse::<i64>().unwrap_or(0) < latest_version.parse::<i64>().unwrap_or(0)
        }))
    }

    /// Get a DAO for code documents

    /// Get a Room Membership DAO for this database
    pub fn room_membership_dao(&self) -> RoomMembershipDao {
        use crate::db::dao::RoomMembershipDao;
        RoomMembershipDao::new(crate::db::client::DatabaseClient::SurrealKV(
            std::sync::Arc::new((*self.db).clone()),
        ))
    }

    /// Get a Message DAO for this database
    pub fn message_dao(&self) -> MessageDao {
        use crate::db::dao::MessageDao;
        MessageDao::new(crate::db::client::DatabaseClient::SurrealKV(
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

/// Create account_data table
async fn create_account_data_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS account_data TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS event_type ON account_data TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS room_id ON account_data TYPE option<string>;
        DEFINE FIELD IF NOT EXISTS event ON account_data TYPE object;
        DEFINE FIELD IF NOT EXISTS updated_at ON account_data TYPE datetime;
        DEFINE INDEX IF NOT EXISTS account_data_lookup ON account_data COLUMNS event_type, room_id;
    ").await?;
    Ok(())
}

/// Create api_cache table
async fn create_api_cache_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS api_cache TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS cache_key ON api_cache TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS response_data ON api_cache TYPE object;
        DEFINE FIELD IF NOT EXISTS expires_at ON api_cache TYPE datetime;
        DEFINE FIELD IF NOT EXISTS created_at ON api_cache TYPE datetime;
        DEFINE INDEX IF NOT EXISTS api_cache_key ON api_cache COLUMNS cache_key UNIQUE;
    ").await?;
    Ok(())
}

/// Create custom_value table
async fn create_custom_value_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS custom_value TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS key ON custom_value TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS value ON custom_value TYPE bytes;
        DEFINE FIELD IF NOT EXISTS created_at ON custom_value TYPE datetime;
        DEFINE FIELD IF NOT EXISTS updated_at ON custom_value TYPE datetime;
        DEFINE INDEX IF NOT EXISTS custom_value_key ON custom_value COLUMNS key UNIQUE;
    ").await?;
    Ok(())
}

/// Create key_value table
async fn create_key_value_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS key_value TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS key ON key_value TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS value ON key_value TYPE object;
        DEFINE FIELD IF NOT EXISTS value_type ON key_value TYPE string;
        DEFINE INDEX IF NOT EXISTS key_value_key ON key_value COLUMNS key UNIQUE;
    ").await?;
    Ok(())
}

/// Create media_upload table
async fn create_media_upload_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS media_upload TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS request_id ON media_upload TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS mxc_uri ON media_upload TYPE option<string>;
        DEFINE FIELD IF NOT EXISTS file_path ON media_upload TYPE string;
        DEFINE FIELD IF NOT EXISTS content_type ON media_upload TYPE string;
        DEFINE FIELD IF NOT EXISTS file_size ON media_upload TYPE int;
        DEFINE FIELD IF NOT EXISTS status ON media_upload TYPE string;
        DEFINE FIELD IF NOT EXISTS created_at ON media_upload TYPE datetime;
        DEFINE FIELD IF NOT EXISTS uploaded_at ON media_upload TYPE option<datetime>;
        DEFINE INDEX IF NOT EXISTS media_upload_request ON media_upload COLUMNS request_id UNIQUE;
    ").await?;
    Ok(())
}

/// Create message table
async fn create_message_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS message TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS room_id ON message TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS sender_id ON message TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS content ON message TYPE string;
        DEFINE FIELD IF NOT EXISTS message_type ON message TYPE string;
        DEFINE FIELD IF NOT EXISTS sent_at ON message TYPE datetime;
        DEFINE FIELD IF NOT EXISTS edited_at ON message TYPE option<datetime>;
        DEFINE FIELD IF NOT EXISTS reactions ON message TYPE array<object>;
        DEFINE INDEX IF NOT EXISTS message_room_time ON message COLUMNS room_id, sent_at;
    ").await?;
    Ok(())
}

/// Create presence table
async fn create_presence_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS presence TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS user_id ON presence TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS presence_event ON presence TYPE object;
        DEFINE FIELD IF NOT EXISTS updated_at ON presence TYPE datetime;
        DEFINE INDEX IF NOT EXISTS presence_user ON presence COLUMNS user_id UNIQUE;
    ").await?;
    Ok(())
}

/// Create receipt table
async fn create_receipt_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS receipt TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS room_id ON receipt TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS user_id ON receipt TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS event_id ON receipt TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS receipt_type ON receipt TYPE string;
        DEFINE FIELD IF NOT EXISTS thread ON receipt TYPE string;
        DEFINE FIELD IF NOT EXISTS receipt_data ON receipt TYPE object;
        DEFINE FIELD IF NOT EXISTS created_at ON receipt TYPE datetime;
        DEFINE INDEX IF NOT EXISTS receipt_lookup ON receipt COLUMNS room_id, user_id, receipt_type, thread;
    ").await?;
    Ok(())
}

/// Create request_dependency table
async fn create_request_dependency_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS request_dependency TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS parent_request_id ON request_dependency TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS child_request_id ON request_dependency TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS dependency_type ON request_dependency TYPE string;
        DEFINE FIELD IF NOT EXISTS created_at ON request_dependency TYPE datetime;
        DEFINE INDEX IF NOT EXISTS request_dependency_parent ON request_dependency COLUMNS parent_request_id;
        DEFINE INDEX IF NOT EXISTS request_dependency_child ON request_dependency COLUMNS child_request_id UNIQUE;
    ").await?;
    Ok(())
}

/// Create room_membership table
async fn create_room_membership_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS room_membership TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS room_id ON room_membership TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS user_id ON room_membership TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS membership_state ON room_membership TYPE string;
        DEFINE FIELD IF NOT EXISTS display_name ON room_membership TYPE option<string>;
        DEFINE FIELD IF NOT EXISTS avatar_url ON room_membership TYPE option<string>;
        DEFINE FIELD IF NOT EXISTS power_level ON room_membership TYPE int;
        DEFINE FIELD IF NOT EXISTS updated_at ON room_membership TYPE datetime;
        DEFINE INDEX IF NOT EXISTS room_membership_lookup ON room_membership COLUMNS room_id, user_id UNIQUE;
    ").await?;
    Ok(())
}

/// Create room_state table
async fn create_room_state_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS room_state TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS room_id ON room_state TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS event_type ON room_state TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS state_key ON room_state TYPE string;
        DEFINE FIELD IF NOT EXISTS event ON room_state TYPE object;
        DEFINE FIELD IF NOT EXISTS updated_at ON room_state TYPE datetime;
        DEFINE INDEX IF NOT EXISTS room_state_lookup ON room_state COLUMNS room_id, event_type, state_key UNIQUE;
    ").await?;
    Ok(())
}

/// Create send_queue table
async fn create_send_queue_table(client: &DatabaseClient) -> Result<()> {
    client.query::<()>("
        DEFINE TABLE IF NOT EXISTS send_queue TYPE NORMAL SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS room_id ON send_queue TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS transaction_id ON send_queue TYPE string ASSERT $value != NONE;
        DEFINE FIELD IF NOT EXISTS event_type ON send_queue TYPE string;
        DEFINE FIELD IF NOT EXISTS content ON send_queue TYPE object;
        DEFINE FIELD IF NOT EXISTS priority ON send_queue TYPE int;
        DEFINE FIELD IF NOT EXISTS error ON send_queue TYPE option<string>;
        DEFINE FIELD IF NOT EXISTS created_at ON send_queue TYPE datetime;
        DEFINE FIELD IF NOT EXISTS last_attempted_at ON send_queue TYPE option<datetime>;
        DEFINE FIELD IF NOT EXISTS attempts ON send_queue TYPE int;
        DEFINE INDEX IF NOT EXISTS send_queue_lookup ON send_queue COLUMNS room_id, transaction_id UNIQUE;
        DEFINE INDEX IF NOT EXISTS send_queue_priority ON send_queue COLUMNS room_id, priority DESC, created_at ASC;
    ").await?;
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

