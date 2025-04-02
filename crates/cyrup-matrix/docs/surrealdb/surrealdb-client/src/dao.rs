use crate::client::DatabaseClient;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use tokio::sync::mpsc::{self, Receiver};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Generic entity trait for database objects
pub trait Entity: Serialize + DeserializeOwned + Debug + Send + Sync + Clone {
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
}

impl<T: Send + 'static> DaoSession<T> {
    /// Create a new session
    fn new(rx: Receiver<DaoResponse<T>>, handle: JoinHandle<()>) -> Self {
        Self {
            rx,
            _handle: handle,
        }
    }

    /// Wait for a single entity
    pub async fn entity(mut self) -> Result<T> {
        match self.rx.recv().await {
            Some(DaoResponse::Entity(entity)) => Ok(entity),
            Some(DaoResponse::Error(err)) => Err(err),
            _ => Err(Error::other("Unexpected response type")),
        }
    }

    /// Wait for an optional entity
    pub async fn optional_entity(mut self) -> Result<Option<T>> {
        match self.rx.recv().await {
            Some(DaoResponse::OptionalEntity(entity)) => Ok(entity),
            Some(DaoResponse::Error(err)) => Err(err),
            _ => Err(Error::other("Unexpected response type")),
        }
    }

    /// Wait for multiple entities
    pub async fn entities(mut self) -> Result<Vec<T>> {
        match self.rx.recv().await {
            Some(DaoResponse::Entities(entities)) => Ok(entities),
            Some(DaoResponse::Error(err)) => Err(err),
            _ => Err(Error::other("Unexpected response type")),
        }
    }

    /// Wait for success
    pub async fn success(mut self) -> Result<()> {
        match self.rx.recv().await {
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
    fn get(&self, id: &str) -> DaoSession<T>;

    /// Update an entity
    fn update(&self, entity: &T) -> DaoSession<T>;

    /// Delete an entity by ID
    fn delete(&self, id: &str) -> DaoSession<T>;

    /// Get all entities
    fn get_all(&self) -> DaoSession<T>;

    /// Create a table for this entity
    fn create_table(&self) -> DaoSession<()>;
}

/// Base Data Access Object for SurrealDB
#[derive(Debug, Clone)]
pub struct Dao<T: Entity> {
    client: DatabaseClient,
    _marker: PhantomData<T>,
}

impl<T: Entity> Dao<T> {
    /// Create a new DAO
    pub fn new(client: DatabaseClient) -> Self {
        Self {
            client,
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
        R: DeserializeOwned + Send + 'static,
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
        R: DeserializeOwned + Send + 'static,
    {
        self.client.query_with_params(query, params).await
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
            // Create the entity
            let result = client.create(T::table_name(), entity_clone.clone()).await;
            match result {
                Ok(created) => {
                    let _ = tx.send(DaoResponse::Entity(created)).await;
                }
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                }
            }
        });

        DaoSession::new(rx, handle)
    }

    /// Get an entity by ID
    fn get(&self, id: &str) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();
        let id = id.to_string();

        let handle = tokio::spawn(async move {
            let id_parts: Vec<&str> = id.split(':').collect();
            if id_parts.len() != 2 {
                let _ = tx
                    .send(DaoResponse::Error(Error::validation(format!(
                        "Invalid ID format: {}",
                        id
                    ))))
                    .await;
                return;
            }

            let table = id_parts[0];
            if table != T::table_name() {
                let _ = tx
                    .send(DaoResponse::Error(Error::validation(format!(
                        "ID {} belongs to table {}, not {}",
                        id,
                        table,
                        T::table_name()
                    ))))
                    .await;
                return;
            }

            match client.get::<T>(T::table_name(), &id).await {
                Ok(entity) => {
                    let _ = tx.send(DaoResponse::OptionalEntity(entity)).await;
                }
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                }
            }
        });

        DaoSession::new(rx, handle)
    }

    /// Update an entity
    fn update(&self, entity: &T) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();
        let entity = entity.clone();

        let handle = tokio::spawn(async move {
            match entity.id() {
                Some(id) => {
                    let id_parts: Vec<&str> = id.split(':').collect();
                    if id_parts.len() != 2 {
                        let _ = tx
                            .send(DaoResponse::Error(Error::validation(format!(
                                "Invalid ID format: {}",
                                id
                            ))))
                            .await;
                        return;
                    }

                    match client
                        .update::<T>(T::table_name(), &id, entity.clone())
                        .await
                    {
                        Ok(updated) => {
                            let _ = tx.send(DaoResponse::OptionalEntity(updated)).await;
                        }
                        Err(err) => {
                            let _ = tx.send(DaoResponse::Error(err)).await;
                        }
                    }
                }
                None => {
                    let _ = tx
                        .send(DaoResponse::Error(Error::validation(
                            "Entity ID is required for update",
                        )))
                        .await;
                }
            }
        });

        DaoSession::new(rx, handle)
    }

    /// Delete an entity by ID
    fn delete(&self, id: &str) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();
        let id = id.to_string();

        let handle = tokio::spawn(async move {
            let id_parts: Vec<&str> = id.split(':').collect();
            if id_parts.len() != 2 {
                let _ = tx
                    .send(DaoResponse::Error(Error::validation(format!(
                        "Invalid ID format: {}",
                        id
                    ))))
                    .await;
                return;
            }

            let table = id_parts[0];
            if table != T::table_name() {
                let _ = tx
                    .send(DaoResponse::Error(Error::validation(format!(
                        "ID {} belongs to table {}, not {}",
                        id,
                        table,
                        T::table_name()
                    ))))
                    .await;
                return;
            }

            match client.delete::<T>(T::table_name(), &id).await {
                Ok(deleted) => {
                    let _ = tx.send(DaoResponse::OptionalEntity(deleted)).await;
                }
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                }
            }
        });

        DaoSession::new(rx, handle)
    }

    /// Get all entities
    fn get_all(&self) -> DaoSession<T> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();

        let handle = tokio::spawn(async move {
            match client.select::<T>(T::table_name()).await {
                Ok(entities) => {
                    let _ = tx.send(DaoResponse::Entities(entities)).await;
                }
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                }
            }
        });

        DaoSession::new(rx, handle)
    }

    /// Create a table for this entity
    fn create_table(&self) -> DaoSession<()> {
        let (tx, rx) = mpsc::channel(1);
        let client = self.client.clone();

        let handle = tokio::spawn(async move {
            let query = format!("DEFINE TABLE {} SCHEMAFULL", T::table_name());
            match client.query::<()>(&query).await {
                Ok(_) => {
                    let _ = tx.send(DaoResponse::Success).await;
                }
                Err(err) => {
                    let _ = tx.send(DaoResponse::Error(err)).await;
                }
            }
        });

        DaoSession::new(rx, handle)
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
        Self {
            id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
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
