# SurrealDB Migration Task List

This document provides a comprehensive, step-by-step guide for migrating Cyrum from SQLite to SurrealDB. Each task is designed to be clear, actionable, and to minimize disruption to the existing codebase.

## Phase 1: Setup and Initial Configuration

### 1. Install SurrealDB Dependencies
- [ ] Add SurrealDB to Cargo.toml
  ```bash
  cargo add surrealdb@2.2.1
  cargo add surrealdb-migrations@2.2.0
  ```
- [ ] Verify compilation with new dependencies
  ```bash
  cargo check
  ```

### 2. Configure SurrealDB Connection
- [ ] Create a global database instance in `src/db/db.rs`
  ```rust
  use std::sync::LazyLock;
  use surrealdb::engine::local::SurrealKv;
  use surrealdb::Surreal;
  
  static DB: LazyLock<Surreal<SurrealKv>> = LazyLock::new(Surreal::init);
  ```
- [ ] Implement connection initialization function in `src/db/client.rs`
  ```rust
  pub async fn init_database(path: &str) -> Result<(), Error> {
      DB.connect(format!("file://{}", path)).await?;
      Ok(())
  }
  ```
- [ ] Add namespace and database selection in `src/db/client.rs`
  ```rust
  pub async fn use_namespace_and_db(namespace: &str, database: &str) -> Result<(), Error> {
      DB.use_ns(namespace).use_db(database).await?;
      Ok(())
  }
  ```

### 3. Update Configuration Management
- [ ] Add SurrealDB configuration to `src/db/config.rs`
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct DbConfig {
      pub engine: StorageEngine,
      pub path: Option<String>,
      pub namespace: String,
      pub database: String,
  }
  
  #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
  pub enum StorageEngine {
      SurrealKv,
      Memory,
  }
  ```
- [ ] Implement validation and path creation
  ```rust
  impl DbConfig {
      pub fn ensure_db_dir(&self) -> std::io::Result<()> {
          if let Some(path) = &self.path {
              if let Some(parent) = std::path::Path::new(path).parent() {
                  std::fs::create_dir_all(parent)?;
              }
          }
          Ok(())
      }
  }
  ```

## Phase 2: Database Abstraction Layer

### 4. Define Base Entity Trait
- [ ] Create the base Entity trait in `src/db/dao.rs`
  ```rust
  pub trait Entity: Serialize + DeserializeOwned + Debug + Send + Sync + Clone {
      fn table_name() -> &'static str;
      fn id(&self) -> Option<String>;
      fn set_id(&mut self, id: String);
  }
  ```

### 5. Implement the DAO Pattern
- [ ] Create a generic DAO implementation in `src/db/dao.rs`
  ```rust
  pub struct Dao<T: Entity> {
      _marker: PhantomData<T>,
  }
  
  impl<T: Entity> Dao<T> {
      pub fn new() -> Self {
          Self { _marker: PhantomData }
      }
      
      pub async fn create(&self, entity: &mut T) -> Result<T> {
          // Implementation
      }
      
      pub async fn get(&self, id: &str) -> Result<Option<T>> {
          // Implementation
      }
      
      // Additional methods: update, delete, query, etc.
  }
  ```

### 6. Implement Transaction Support
- [ ] Add transaction methods in `src/db/client.rs`
  ```rust
  pub async fn begin_transaction() -> Result<()> {
      DB.query("BEGIN TRANSACTION").await?.check()?;
      Ok(())
  }
  
  pub async fn commit_transaction() -> Result<()> {
      DB.query("COMMIT TRANSACTION").await?.check()?;
      Ok(())
  }
  
  pub async fn rollback_transaction() -> Result<()> {
      DB.query("ROLLBACK TRANSACTION").await?.check()?;
      Ok(())
  }
  ```

## Phase 3: Schema and Migrations

### 7. Define Migration System
- [ ] Create migration infrastructure in `src/db/migrations.rs`
  ```rust
  pub struct Migration {
      pub version: String,
      pub name: String,
      pub sql: String,
  }
  
  pub async fn run_migrations(migrations: Vec<Migration>) -> Result<()> {
      // Implementation
  }
  ```
- [ ] Define schema versioning table
  ```rust
  pub async fn initialize_migration_table() -> Result<()> {
      DB.query("
          DEFINE TABLE IF NOT EXISTS migrations SCHEMAFULL;
          DEFINE FIELD version ON migrations TYPE string;
          DEFINE FIELD name ON migrations TYPE string;
          DEFINE FIELD executed_at ON migrations TYPE datetime;
      ").await?;
      Ok(())
  }
  ```

### 8. Create Entity Schema Definitions
- [ ] Define room membership table and indexes
  ```rust
  const ROOM_MEMBERSHIP_SCHEMA: &str = r#"
      DEFINE TABLE room_membership SCHEMAFULL;
      DEFINE FIELD user_id ON room_membership TYPE string;
      DEFINE FIELD room_id ON room_membership TYPE string;
      DEFINE FIELD display_name ON room_membership TYPE option<string>;
      DEFINE FIELD membership_status ON room_membership TYPE string;
      DEFINE FIELD joined_at ON room_membership TYPE datetime;
      DEFINE FIELD updated_at ON room_membership TYPE datetime;
      
      DEFINE INDEX room_membership_user_idx ON room_membership COLUMNS user_id;
      DEFINE INDEX room_membership_room_idx ON room_membership COLUMNS room_id;
  "#;
  ```
- [ ] Define message history table and indexes
  ```rust
  const MESSAGE_SCHEMA: &str = r#"
      DEFINE TABLE message SCHEMAFULL;
      DEFINE FIELD room_id ON message TYPE string;
      DEFINE FIELD sender_id ON message TYPE string;
      DEFINE FIELD content ON message TYPE string;
      DEFINE FIELD message_type ON message TYPE string;
      DEFINE FIELD sent_at ON message TYPE datetime;
      DEFINE FIELD edited_at ON message TYPE option<datetime>;
      DEFINE FIELD reactions ON message TYPE array<object>;
      
      DEFINE INDEX message_time_idx ON message COLUMNS room_id, sent_at;
      DEFINE INDEX message_sender_idx ON message COLUMNS sender_id, sent_at;
  "#;
  ```
- [ ] Define user profile table and indexes
  ```rust
  const USER_PROFILE_SCHEMA: &str = r#"
      DEFINE TABLE user_profile SCHEMAFULL;
      DEFINE FIELD user_id ON user_profile TYPE string;
      DEFINE FIELD display_name ON user_profile TYPE option<string>;
      DEFINE FIELD avatar_url ON user_profile TYPE option<string>;
      DEFINE FIELD email ON user_profile TYPE option<string>;
      DEFINE FIELD presence ON user_profile TYPE string;
      DEFINE FIELD last_active ON user_profile TYPE datetime;
      DEFINE FIELD devices ON user_profile TYPE array<object>;
      DEFINE FIELD settings ON user_profile TYPE object;
      
      DEFINE INDEX user_profile_id_idx ON user_profile COLUMNS user_id UNIQUE;
  "#;
  ```
- [ ] Define API cache table and indexes
  ```rust
  const API_CACHE_SCHEMA: &str = r#"
      DEFINE TABLE api_cache SCHEMAFULL;
      DEFINE FIELD endpoint ON api_cache TYPE string;
      DEFINE FIELD parameters ON api_cache TYPE object;
      DEFINE FIELD response_data ON api_cache TYPE any;
      DEFINE FIELD cached_at ON api_cache TYPE datetime;
      DEFINE FIELD expires_at ON api_cache TYPE option<datetime>;
      DEFINE FIELD etag ON api_cache TYPE option<string>;
      
      DEFINE INDEX api_cache_endpoint_idx ON api_cache COLUMNS endpoint, parameters;
      DEFINE INDEX api_cache_expiry_idx ON api_cache COLUMNS expires_at;
  "#;
  ```
- [ ] Define encryption data table and indexes
  ```rust
  const ENCRYPTION_DATA_SCHEMA: &str = r#"
      DEFINE TABLE encryption_data SCHEMAFULL;
      DEFINE FIELD user_id ON encryption_data TYPE string;
      DEFINE FIELD device_id ON encryption_data TYPE string;
      DEFINE FIELD keys ON encryption_data TYPE object;
      DEFINE FIELD signatures ON encryption_data TYPE object;
      DEFINE FIELD verification_status ON encryption_data TYPE string;
      DEFINE FIELD updated_at ON encryption_data TYPE datetime;
      
      DEFINE INDEX encryption_user_device_idx ON encryption_data COLUMNS user_id, device_id UNIQUE;
  "#;
  ```
- [ ] Define searchable message table with vector and full-text indexes
  ```rust
  const SEARCHABLE_MESSAGE_SCHEMA: &str = r#"
      DEFINE TABLE searchable_message SCHEMAFULL;
      DEFINE FIELD message_id ON searchable_message TYPE string;
      DEFINE FIELD room_id ON searchable_message TYPE string;
      DEFINE FIELD sender_id ON searchable_message TYPE string;
      DEFINE FIELD content ON searchable_message TYPE string;
      DEFINE FIELD sent_at ON searchable_message TYPE datetime;
      DEFINE FIELD embedding ON searchable_message TYPE array<float>;
      
      DEFINE INDEX message_content_idx ON searchable_message FULLTEXT content;
      DEFINE INDEX message_vector_idx ON searchable_message VECTOR embedding 384 cosine;
  "#;
  ```

### 9. Create Initial Migration
- [ ] Bundle all schema definitions into an initial migration
  ```rust
  pub fn get_initial_migration() -> Migration {
      Migration {
          version: "20250329_000000".to_string(),
          name: "Initial schema".to_string(),
          sql: format!("{}\n{}\n{}\n{}\n{}\n{}",
              ROOM_MEMBERSHIP_SCHEMA,
              MESSAGE_SCHEMA,
              USER_PROFILE_SCHEMA,
              API_CACHE_SCHEMA,
              ENCRYPTION_DATA_SCHEMA,
              SEARCHABLE_MESSAGE_SCHEMA
          ),
      }
  }
  ```

## Phase 4: Entity Implementation

### 10. Define Data Structures for Entities
- [ ] Create Room Membership structure in appropriate module
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct RoomMembership {
      pub id: Option<String>,
      pub user_id: String,
      pub room_id: String,
      pub display_name: Option<String>,
      pub membership_status: String,
      pub joined_at: DateTime<Utc>,
      pub updated_at: DateTime<Utc>,
  }
  
  impl Entity for RoomMembership {
      fn table_name() -> &'static str {
          "room_membership"
      }
      
      fn id(&self) -> Option<String> {
          self.id.clone()
      }
      
      fn set_id(&mut self, id: String) {
          self.id = Some(id);
      }
  }
  ```
- [ ] Implement remaining entity structures (Message, UserProfile, etc.)

### 11. Create DAO Implementations
- [ ] Implement RoomMembershipDao
  ```rust
  pub struct RoomMembershipDao {
      dao: Dao<RoomMembership>,
  }
  
  impl RoomMembershipDao {
      pub fn new() -> Self {
          Self { dao: Dao::new() }
      }
      
      pub async fn find_by_room(&self, room_id: &str) -> Result<Vec<RoomMembership>> {
          self.dao.query_with_params(
              "SELECT * FROM room_membership WHERE room_id = $room",
              json!({ "room": room_id })
          ).await
      }
      
      // Additional specialized methods
  }
  ```
- [ ] Implement remaining DAOs for other entities

## Phase 5: Migration from SQLite

### 12. Create Data Migration Tool
- [ ] Implement a function to export data from SQLite
  ```rust
  pub async fn export_from_sqlite(sqlite_path: &str) -> Result<ExportData> {
      // Implementation
  }
  ```
- [ ] Implement a function to import data into SurrealDB
  ```rust
  pub async fn import_to_surrealdb(data: ExportData) -> Result<()> {
      // Implementation
  }
  ```

### 13. Define Migration Logic
- [ ] Add user-prompted migration check in application startup
  ```rust
  pub async fn check_migration_needed(
      sqlite_path: &Path, 
      surreal_path: &Path
  ) -> Result<bool> {
      // Implementation
  }
  ```
- [ ] Implement migration confirmation dialog in UI

## Phase 6: Integration Testing

### 14. Write Test Suite
- [ ] Create integration tests for database operations
  ```rust
  #[tokio::test]
  async fn test_room_membership_crud() {
      // Implementation
  }
  ```
- [ ] Create tests for data migration process
  ```rust
  #[tokio::test]
  async fn test_sqlite_to_surreal_migration() {
      // Implementation
  }
  ```

### 15. Create Database Reset Tool for Testing
- [ ] Implement function to clear and reset database
  ```rust
  pub async fn reset_test_database() -> Result<()> {
      // Implementation
  }
  ```

## Phase 7: Application Integration

### 16. Update Application Initialization
- [ ] Modify main.rs to initialize SurrealDB
  ```rust
  async fn init() -> Result<()> {
      let config = load_config()?;
      init_database(&config.db_path).await?;
      use_namespace_and_db(&config.namespace, &config.database).await?;
      run_migrations(get_all_migrations()).await?;
      Ok(())
  }
  ```

### 17. Update Error Handling
- [ ] Modify error.rs to include SurrealDB errors
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum Error {
      #[error("SurrealDB error: {0}")]
      Database(#[from] surrealdb::Error),
      // Other error variants
  }
  ```

### 18. Implement Graceful Fallback
- [ ] Add logic to fall back to SQLite if SurrealDB initialization fails
  ```rust
  async fn init_database_with_fallback() -> Result<()> {
      match init_surrealdb().await {
          Ok(_) => Ok(()),
          Err(e) => {
              println!("Failed to initialize SurrealDB: {}", e);
              println!("Falling back to SQLite");
              init_sqlite().await
          }
      }
  }
  ```

## Phase 8: Performance Optimization

### 19. Optimize Query Performance
- [ ] Add index creation for commonly used queries
- [ ] Implement query caching where appropriate
- [ ] Analyze and optimize complex queries

### 20. Implement Batch Operations
- [ ] Add bulk insert/update methods to DAOs
  ```rust
  pub async fn bulk_insert(&self, entities: Vec<T>) -> Result<Vec<T>> {
      // Implementation
  }
  ```

## Phase 9: Documentation and Deployment

### 21. Update Documentation
- [ ] Add SurrealDB documentation to codebase
- [ ] Update README with new database information
- [ ] Document the migration process for end users

### 22. Create Database Backup Tool
- [ ] Implement function to create database backups
  ```rust
  pub async fn backup_database(path: &str) -> Result<()> {
      // Implementation
  }
  ```

### 23. Finalize Release
- [ ] Test migration on various platforms
- [ ] Create release notes explaining the change
- [ ] Plan for supporting users through the transition

## Additional Recommendations

1. **Progressive Rollout**:
   - Consider implementing a feature flag to enable/disable SurrealDB
   - Allow users to opt-in to the migration initially

2. **Data Validation**:
   - Add comprehensive validation of migrated data
   - Implement integrity checks after migration

3. **Performance Monitoring**:
   - Add telemetry to compare performance before and after migration
   - Monitor for any regressions

4. **User Feedback**:
   - Establish a feedback mechanism for users experiencing migration issues
   - Prepare common troubleshooting steps for support

This task list provides a comprehensive roadmap for implementing SurrealDB in the Cyrum project. Each step builds on the previous ones, allowing for incremental development and testing.