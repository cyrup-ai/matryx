use crate::db::client::DatabaseClient;
use crate::db::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use tokio::sync::mpsc::{self, Receiver};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Generic entity trait for database objects
pub trait Entity: Serialize + DeserializeOwned + Debug + Send + Sync + Clone + 'static {
    /// Get the table name for this entity
    fn table_name() -> &'static str;

    /// Get the ID of this entity
    fn id(&self) -> Option<String>;

    /// Set the ID of this entity
    fn set_id(&mut self, id: String);

    /// Generate a unique ID for this entity
    fn generate_id() -> String {
        format!("{}:{}", Self::table_name(), Uuid::new_v4())
    }
}

/// Response type for DAO operations
pub enum DaoResponse<T> {
    /// A single entity
    Entity(T),
    /// An optional entity
    OptionalEntity(Option<T>),
    /// Multiple entities
    Entities(Vec<T>),
    /// Success with no data
    Success,
    /// Error
    Error(Error),
}

/// Session type for async DAO operations
pub struct DaoSession<T: Send + 'static> {
    rx: Receiver<DaoResponse<T>>,
    _handle: JoinHandle<()>,
    mode: DaoSessionMode,
}

/// Mode for the DAO session
enum DaoSessionMode {
    Entity,
    OptionalEntity,
    Entities,
    Success,
}

impl<T: Send + 'static> DaoSession<T> {
    /// Create a new session for entity
    fn new_entity(rx: Receiver<DaoResponse<T>>, handle: JoinHandle<()>) -> Self {
        Self { rx, _handle: handle, mode: DaoSessionMode::Entity }
    }

    /// Create a new session for optional entity
    fn new_optional_entity(rx: Receiver<DaoResponse<T>>, handle: JoinHandle<()>) -> Self {
        Self {
            rx,
            _handle: handle,
            mode: DaoSessionMode::OptionalEntity,
        }
    }

    /// Create a new session for entities
    fn new_entities(rx: Receiver<DaoResponse<T>>, handle: JoinHandle<()>) -> Self {
        Self {
            rx,
            _handle: handle,
            mode: DaoSessionMode::Entities,
        }
    }

    /// Create a new session for success
    fn new_success(rx: Receiver<DaoResponse<T>>, handle: JoinHandle<()>) -> Self {
        Self { rx, _handle: handle, mode: DaoSessionMode::Success }
    }
}

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

impl<T: Send + 'static> Future for DaoSession<T> {
    type Output = Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(DaoResponse::Entity(entity))) => {
                match self.mode {
                    DaoSessionMode::Entity => Poll::Ready(Ok(entity)),
                    _ => Poll::Ready(Err(Error::other("Unexpected response type"))),
                }
            },
            Poll::Ready(Some(DaoResponse::Error(err))) => Poll::Ready(Err(err)),
            Poll::Ready(None) => Poll::Ready(Err(Error::other("Channel closed unexpectedly"))),
            Poll::Ready(Some(_)) => Poll::Ready(Err(Error::other("Unexpected response type"))),
            Poll::Pending => Poll::Pending,
        }
    }
}

// Add additional Future implementations for other response types
impl<T: Send + 'static> DaoSession<T> {
    /// Wait for an optional entity
    pub async fn optional_entity(self) -> Result<Option<T>> {
        let mut rx = self.rx;
        match rx.recv().await {
            Some(DaoResponse::OptionalEntity(entity)) => Ok(entity),
            Some(DaoResponse::Error(err)) => Err(err),
            _ => Err(Error::other("Unexpected response type")),
        }
    }

    /// Wait for multiple entities
    pub async fn entities(self) -> Result<Vec<T>> {
        let mut rx = self.rx;
        match rx.recv().await {
            Some(DaoResponse::Entities(entities)) => Ok(entities),
            Some(DaoResponse::Error(err)) => Err(err),
            _ => Err(Error::other("Unexpected response type")),
        }
    }

    /// Wait for success
    pub async fn success(self) -> Result<()> {
        let mut rx = self.rx;
        match rx.recv().await {
            Some(DaoResponse::Success) => Ok(()),
            Some(DaoResponse::Error(err)) => Err(err),
            _ => Err(Error::other("Unexpected response type")),
        }
    }
}

/// Base DAO trait providing common CRUD operations for entities
pub trait BaseDao<T: Entity + 'static> {
    /// Create a new entity
    fn create(&self, entity: &mut T) -> DaoSession<T>;

    /// Get an entity by ID
    fn get(&self, id: String) -> DaoSession<T>;

    /// Update an entity
    fn update(&self, entity: &T) -> DaoSession<T>;

    /// Delete an entity by ID
    fn delete(&self, id: String) -> DaoSession<T>;

    /// Get all entities
    fn get_all(&self) -> DaoSession<T>;

    /// Create a table for this entity
    fn create_table(&self) -> DaoSession<()>;
}

/// Base Data Access Object for SurrealDB
#[derive(Debug, Clone)]
pub struct Dao<T: Entity> {
    client: DatabaseClient,
    table: String,
    _marker: PhantomData<T>,
}

impl<T: Entity> Dao<T> {
    /// Create a new DAO
    pub fn new(client: DatabaseClient, table: &str) -> Self {
        Self {
            client,
            table: table.to_string(),
            _marker: PhantomData,
        }
    }

    /// Get the client reference
    pub fn client(&self) -> &DatabaseClient {
        &self.client
    }

    /// Execute an arbitrary query
    pub async fn query<R>(&self, query: &str) -> Result<R>
    where
        R: DeserializeOwned + Send + Sync + 'static,
    {
        self.client.query(query).await
    }

    /// Execute a query with parameters
    pub async fn query_with_params<R>(
        &self,
        query: &str,
        params: impl Serialize + Clone + Send + 'static,
    ) -> Result<R>
    where
        R: DeserializeOwned + Send + Sync + 'static,
    {
        let query_stream = self.client.query_with_params(query, params)?;
        Ok(query_stream.await?)
    }

    /// Get transaction manager
    pub fn transaction(&self) -> crate::db::client::TransactionManager {
        self.client.transaction()
    }

    /// Create a live query
    pub fn live_query<R>(&self, query: &str) -> crate::db::client::LiveQueryStream<R>
    where
        R: DeserializeOwned + Send + Sync + 'static,
    {
        self.client.live_query(query)
    }

    /// Find an entity by its ID
    pub async fn find_by_id(&self, id: impl Into<String>) -> Result<Option<T>> {
        let id_str = id.into();
        self.client.get(&self.table, &id_str).await
    }

    /// Find an entity by a field value
    pub async fn find_by_field(&self, field: impl Into<String>, value: impl Into<String>) -> Result<Option<T>> {
        let field_str = field.into();
        let value_str = value.into();
        let query = format!("SELECT * FROM {} WHERE {} = $value LIMIT 1", self.table, field_str);
        let params = serde_json::json!({ "value": value_str });
        
        let items: Vec<T> = self.query_with_params(&query, params).await?;
        Ok(items.into_iter().next())
    }

    /// Create a new entity
    pub async fn create(&self, entity: &T) -> Result<T> {
        self.client.create(&self.table, entity).await
    }

    /// Update an existing entity
    pub async fn update(&self, entity: &T) -> Result<T> {
        match entity.id() {
            Some(id) => {
                let result = self.client.update(&self.table, &id, entity).await?;
                result.ok_or_else(|| Error::other(format!("Entity not found for update: {}", id)))
            },
            None => Err(Error::other(format!("Entity has no ID"))),
        }
    }

    /// Delete an entity
    pub async fn delete(&self, id: impl Into<String>) -> Result<Option<T>> {
        let id_str = id.into();
        self.client.delete(&self.table, &id_str).await
    }

    /// Execute a raw query
    pub async fn query_raw<R>(&self, query: &str) -> Result<Vec<R>> 
    where 
        R: DeserializeOwned + Send + Sync + 'static,
    {
        self.client.query(query).await
    }

    /// Execute a query with parameters
    pub async fn query_with_params_raw<R>(&self, query: &str, params: serde_json::Value) -> Result<Vec<R>> 
    where 
        R: DeserializeOwned + Send + Sync + 'static,
    {
        let stream = self.client.query_with_params(query, params)?;
        let results: Vec<R> = stream.await?;
        Ok(results)
    }

    /// Create the table for this DAO if it doesn't exist
    pub async fn create_table(&self) -> Result<()> {
        // Basic table creation query
        let query = format!(
            "DEFINE TABLE {} SCHEMAFULL;
             DEFINE FIELD id ON TABLE {} TYPE string;
             DEFINE INDEX {}_id_idx ON TABLE {} COLUMNS id UNIQUE;",
            self.table, self.table, self.table, self.table
        );
        
        // Execute the creation query
        let _: Vec<serde_json::Value> = self.query_raw(&query).await?;
        Ok(())
    }
}

impl<T: Entity + 'static> BaseDao<T> for Dao<T> {
    /// Create a new entity
    fn create(&self, entity: &mut T) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();
        // Clone the entity for the async task
        let mut entity_clone = entity.clone();

        // Generate ID if not provided
        if entity_clone.id().is_none() {
            let generated_id = T::generate_id();
            entity_clone.set_id(generated_id.clone());
            // Update the original entity with the generated ID
            entity.set_id(generated_id);
        }

        let handle = tokio::spawn(async move {
            // Create the entity using client stream
            // For SurrealDB 2.3.3, ensure we're handling the entity correctly
            let result = client.create::<T>(T::table_name(), entity_clone.clone()).await;
            match result {
                Ok(created) => {
                    let _ = tx.send(DaoResponse::Entity(created)).await;
                },
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                },
            }
        });

        DaoSession::new_entity(rx, handle)
    }

    /// Get an entity by ID
    fn get(&self, id: String) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();

        let handle = tokio::spawn(async move {
            // Get the entity using client stream
            let result = client.get::<T>(T::table_name(), &id).await;
            match result {
                Ok(entity) => {
                    let _ = tx.send(DaoResponse::OptionalEntity(entity)).await;
                },
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                },
            }
        });

        DaoSession::new_optional_entity(rx, handle)
    }

    /// Update an entity
    fn update(&self, entity: &T) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();
        
        // Create a mutable clone so we can update the timestamp
        // In Rust we can't just cast to &mut BaseEntity so we'll need to handle
        // timestamp updates in the entity implementation
        let entity_clone = entity.clone();

        let handle = tokio::spawn(async move {
            // Get the ID from the entity
            let id = match entity_clone.id() {
                Some(id) => id,
                None => {
                    let _ = tx
                        .send(DaoResponse::Error(Error::validation(
                            "Cannot update entity without ID",
                        )))
                        .await;
                    return;
                },
            };

            // Extract the actual ID part after the table name
            // This ensures we handle IDs correctly for SurrealDB 2.3.3
            let id_parts: Vec<&str> = id.split(':').collect();
            let id_part = if id_parts.len() > 1 {
                id_parts[1].to_string()
            } else {
                id.clone()
            };

            // Update using client stream with SurrealDB 2.3.3 compatibility
            let result = client.update::<T>(T::table_name(), &id_part, entity_clone).await;
            match result {
                Ok(Some(updated)) => {
                    let _ = tx.send(DaoResponse::Entity(updated)).await;
                },
                Ok(None) => {
                    let _ = tx
                        .send(DaoResponse::Error(Error::not_found(format!(
                            "Entity with ID {} not found",
                            id
                        ))))
                        .await;
                },
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                },
            }
        });

        DaoSession::new_entity(rx, handle)
    }

    /// Delete an entity by ID
    fn delete(&self, id: String) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();

        let handle = tokio::spawn(async move {
            // Extract the actual ID part after the table name
            let id_parts: Vec<&str> = id.split(':').collect();
            let id_part = if id_parts.len() > 1 {
                id_parts[1].to_string()
            } else {
                id.clone()
            };

            // Delete using client stream
            let result = client.delete::<T>(T::table_name(), &id_part).await;
            match result {
                Ok(Some(deleted)) => {
                    let _ = tx.send(DaoResponse::Entity(deleted)).await;
                },
                Ok(None) => {
                    let _ = tx
                        .send(DaoResponse::Error(Error::not_found(format!(
                            "Entity with ID {} not found",
                            id
                        ))))
                        .await;
                },
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                },
            }
        });

        DaoSession::new_entity(rx, handle)
    }

    /// Get all entities
    fn get_all(&self) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();

        let handle = tokio::spawn(async move {
            // Select all using client stream
            let result = client.select::<T>(T::table_name()).await;
            match result {
                Ok(entities) => {
                    let _ = tx.send(DaoResponse::Entities(entities)).await;
                },
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                },
            }
        });

        DaoSession::new_entities(rx, handle)
    }

    /// Create a table for this entity
    fn create_table(&self) -> DaoSession<()> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();

        let handle = tokio::spawn(async move {
            let query = format!("DEFINE TABLE {} SCHEMAFULL", T::table_name());
            let result = client.query(&query).await;
            match result {
                Ok(_) => {
                    let _ = tx.send(DaoResponse::Success).await;
                },
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                },
            }
        });

        DaoSession::new_success(rx, handle)
    }
}

/// Common fields for database entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseEntity {
    /// Entity ID
    pub id: Option<String>,

    /// Creation timestamp
    #[serde(default = "utc_now")]
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    #[serde(default = "utc_now")]
    pub updated_at: DateTime<Utc>,
}

impl BaseEntity {
    /// Create a new entity
    pub fn new() -> Self {
        let now = utc_now();
        Self {
            id: None,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Update the entity's update timestamp
    pub fn touch(&mut self) {
        self.updated_at = utc_now();
    }
}

impl Default for BaseEntity {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to get the current UTC time
fn utc_now() -> DateTime<Utc> {
    Utc::now()
}

/// Example user entity implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(flatten)]
    base: BaseEntity,

    pub username: String,
    pub email: String,
    pub password_hash: Option<String>,
}

impl Entity for User {
    fn table_name() -> &'static str {
        "users"
    }

    fn id(&self) -> Option<String> {
        self.base.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.base.id = Some(id);
    }
}

// User implementation intentionally left without additional methods
// as it serves as an example Entity implementation
