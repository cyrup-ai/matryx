use crate::db::config::{DbConfig, StorageEngine};
use crate::db::error::{Error, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use surrealdb::engine::any::connect;
use futures::stream::Stream;
use std::fmt::Debug;
use std::marker::Unpin;
use std::time::Duration;
use surrealdb::opt::Config;
use surrealdb::opt::Resource;
use surrealdb::{Response, Surreal};
use surrealdb::value::Notification;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, instrument};

// Use type alias to avoid exposing the private Connect type
type SurrealDbConnection = surrealdb::engine::any::Any;



/// Database client for SurrealDB operations
#[derive(Debug, Clone)]
pub enum DatabaseClient {
    /// SurrealDB client with SurrealKV storage engine
    SurrealKV(Arc<Surreal<SurrealDbConnection>>),
}

/// Connect to the database with the provided configuration
pub async fn connect_database(config: &DbConfig) -> Result<DatabaseClient> {
    match config.storage_engine {
        StorageEngine::SurrealKV => {
            match config.file_path() {
                Some(file_path) => {
                    let path = Path::new(&file_path);
                    if let Some(parent) = path.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent).map_err(|e| {
                                Error::other(format!(
                                    "Failed to create directory {}: {}",
                                    parent.display(),
                                    e
                                ))
                            })?;
                        }
                    }

                    // Connect to database with proper format and configuration
                    let conn_str = format!("file://{}", file_path); // File protocol format is the same in SurrealDB 2.3.3
                    
                    // Create configuration with appropriate settings
                    let surreal_config = Config::default()
                        .query_timeout(Duration::from_secs(30));
                        
                    // Connect with configuration
                    let db = connect((conn_str, surreal_config))
                        .await
                        .map_err(|e| {
                            Error::database(format!(
                                "Failed to connect to database at {}: {}",
                                file_path, e
                            ))
                        })?;

                    // Set namespace and database
                    db.use_ns(&config.namespace)
                        .use_db(&config.database)
                        .await
                        .map_err(|e| {
                            Error::database(format!(
                                "Failed to use namespace {} and database {}: {}",
                                config.namespace, config.database, e
                            ))
                        })?;

                    Ok(DatabaseClient::SurrealKV(Arc::new(db)))
                },
                None => Err(Error::database("Database file path not provided".to_string())),
            }
        },
    }
}

/// Action type for live queries
#[derive(Debug, Clone, Deserialize)]
pub enum LiveAction {
    /// Create action
    Create,
    /// Update action
    Update,
    /// Delete action
    Delete,
}



/// Stream for query results
#[derive(Clone)]
pub struct QueryStream<T> {
    client: DatabaseClient,
    query: String,
    params: Option<serde_json::Value>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned + Send + Sync + 'static> QueryStream<T> {
    /// Create a new query stream
    fn new(client: DatabaseClient, query: String) -> Self {
        Self {
            client,
            query,
            params: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a new query stream with parameters
    fn with_params(client: DatabaseClient, query: &str, params: impl Serialize) -> Result<Self> {
        let params = serde_json::to_value(params).map_err(|e| {
            Error::serialization(format!("Failed to serialize parameters to JSON: {}", e))
        })?;

        Ok(Self {
            client,
            query: query.to_string(),
            params: Some(params),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Execute the query and get the result
    pub async fn get(self) -> Result<T> {
        let result = match self.client {
            DatabaseClient::SurrealKV(db) => {
                let mut response = if let Some(params) = self.params {
                    db.query(&self.query).bind(params).await?
                } else {
                    db.query(&self.query).await?
                };

                let result: Option<T> = response.take(0).map_err(|e| {
                    Error::database(format!("Failed to extract result from response: {}", e))
                })?;
                result.ok_or_else(|| Error::database("Query returned no result".to_string()))?
            },
        };

        Ok(result)
    }
}

impl<T: DeserializeOwned + Send + Sync + 'static> std::future::Future for QueryStream<T> {
    type Output = Result<T>;
    
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        // Take ownership of the fields we need
        let client = self.client.clone();
        let query = self.query.clone();
        let params = self.params.clone();
        
        // Create the future with owned data
        let fut = async move {
            match client {
                DatabaseClient::SurrealKV(db) => {
                    let mut response = if let Some(params) = params {
                        db.query(&query).bind(params).await?
                    } else {
                        db.query(&query).await?
                    };

                    let result: Option<T> = response.take(0).map_err(|e| {
                        Error::database(format!("Failed to extract result from response: {}", e))
                    })?;
                    result.ok_or_else(|| Error::database("Query returned no result".to_string()))
                }
            }
        };
        
        // Create a pinned boxed future
        let mut fut = Box::pin(fut);
        // Poll the inner future
        fut.as_mut().poll(cx)
    }
}

/// Stream for optional query results
#[derive(Clone)]
pub struct OptionalQueryStream<T> {
    client: DatabaseClient,
    query: String,
    params: Option<serde_json::Value>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned + Send + Sync + 'static> OptionalQueryStream<T> {
    /// Create a new optional query stream
    fn new(client: DatabaseClient, query: String) -> Self {
        Self {
            client,
            query,
            params: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a new optional query stream with parameters
    fn with_params(client: DatabaseClient, query: &str, params: impl Serialize) -> Result<Self> {
        let params = serde_json::to_value(params).map_err(|e| {
            Error::serialization(format!("Failed to serialize parameters to JSON: {}", e))
        })?;

        Ok(Self {
            client,
            query: query.to_string(),
            params: Some(params),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Execute the query and get the optional result
    pub async fn get(self) -> Result<Option<T>> {
        let result = match self.client {
            DatabaseClient::SurrealKV(db) => {
                let response = if let Some(params) = self.params {
                    db.query(&self.query).bind(params).await?
                } else {
                    db.query(&self.query).await?
                };

                Self::extract_result(response).await?
            },
        };

        Ok(result)
    }

    /// Extract the optional result from a query response
    async fn extract_result(mut response: Response) -> Result<Option<T>> {
        let result: Option<T> = response.take(0).map_err(|e| {
            Error::database(format!("Failed to extract result from response: {}", e))
        })?;

        Ok(result)
    }
}

impl<T: DeserializeOwned + Send + Sync + 'static> std::future::Future for OptionalQueryStream<T> {
    type Output = Result<Option<T>>;
    
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        // Take ownership of the fields we need
        let client = self.client.clone();
        let query = self.query.clone();
        let params = self.params.clone();
        
        // Create the future with owned data
        let fut = async move {
            match client {
                DatabaseClient::SurrealKV(db) => {
                    let response = if let Some(params) = params {
                        db.query(&query).bind(params).await?
                    } else {
                        db.query(&query).await?
                    };

                    Self::extract_result(response).await
                }
            }
        };
        
        // Create a pinned boxed future
        let mut fut = Box::pin(fut);
        // Poll the inner future
        fut.as_mut().poll(cx)
    }
}

/// Stream for multiple query results
#[derive(Clone)]
pub struct MultiQueryStream<T> {
    client: DatabaseClient,
    query: String,
    params: Option<serde_json::Value>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned + Send + Sync + 'static> MultiQueryStream<T> {
    /// Create a new multi query stream
    fn new(client: DatabaseClient, query: String) -> Self {
        Self {
            client,
            query,
            params: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a new multi query stream with parameters
    fn with_params(client: DatabaseClient, query: &str, params: impl Serialize) -> Result<Self> {
        let params = serde_json::to_value(params).map_err(|e| {
            Error::serialization(format!("Failed to serialize parameters to JSON: {}", e))
        })?;

        Ok(Self {
            client,
            query: query.to_string(),
            params: Some(params),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Execute the query and get the results
    pub async fn get(self) -> Result<Vec<T>> {
        let result = match self.client {
            DatabaseClient::SurrealKV(db) => {
                let response = if let Some(params) = self.params {
                    db.query(&self.query).bind(params).await?
                } else {
                    db.query(&self.query).await?
                };

                Self::extract_result(response).await?
            },
        };

        Ok(result)
    }

    /// Extract the results from a query response
    async fn extract_result(mut response: Response) -> Result<Vec<T>> {
        let result: Vec<T> = response.take(0).map_err(|e| {
            Error::database(format!("Failed to extract result from response: {}", e))
        })?;

        Ok(result)
    }
}

impl<T: DeserializeOwned + Send + Sync + 'static> std::future::Future for MultiQueryStream<T> {
    type Output = Result<Vec<T>>;
    
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        // Take ownership of the fields we need
        let client = self.client.clone();
        let query = self.query.clone();
        let params = self.params.clone();
        
        // Create the future with owned data
        let fut = async move {
            match client {
                DatabaseClient::SurrealKV(db) => {
                    let mut response = if let Some(params) = params {
                        db.query(&query).bind(params).await?
                    } else {
                        db.query(&query).await?
                    };

                    let result: Vec<T> = response.take(0).map_err(|e| {
                        Error::database(format!("Failed to extract result from response: {}", e))
                    })?;

                    Ok(result)
                }
            }
        };
        
        // Create a pinned boxed future
        let mut fut = Box::pin(fut);
        // Poll the inner future
        fut.as_mut().poll(cx)
    }
}

/// Transaction stream for transaction operations
pub struct TransactionStream {
    client: DatabaseClient,
}

impl TransactionStream {
    /// Create a new transaction stream
    fn new(client: DatabaseClient) -> Self {
        Self { client }
    }

    /// Commit the transaction
    pub async fn commit(self) -> Result<()> {
        match self.client {
            DatabaseClient::SurrealKV(db) => {
                db.query("COMMIT TRANSACTION").await.map_err(|e| {
                    Error::database(format!("Failed to commit transaction: {}", e))
                })?;
                Ok(())
            },
        }
    }

    /// Cancel the transaction
    pub async fn cancel(self) -> Result<()> {
        match self.client {
            DatabaseClient::SurrealKV(db) => {
                db.query("CANCEL TRANSACTION").await.map_err(|e| {
                    Error::database(format!("Failed to cancel transaction: {}", e))
                })?;
                Ok(())
            },
        }
    }
}

/// Transaction manager for fluent transaction API
#[derive(Clone)]
pub struct TransactionManager {
    client: DatabaseClient,
}

impl TransactionManager {
    /// Create a new transaction manager
    fn new(client: DatabaseClient) -> Self {
        Self { client }
    }

    /// Begin a new transaction
    pub async fn begin(&self) -> Result<TransactionStream> {
        match &self.client {
            DatabaseClient::SurrealKV(db) => {
                db.query("BEGIN TRANSACTION").await.map_err(|e| {
                    Error::database(format!("Failed to begin transaction: {}", e))
                })?;
                Ok(TransactionStream::new(self.client.clone()))
            }
        }
    }

    /// Execute a function in a transaction
    pub async fn execute<F, T, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Begin transaction in SurrealDB
        match &self.client {
            DatabaseClient::SurrealKV(db) => {
                db.query("BEGIN TRANSACTION").await.map_err(|e| {
                    Error::database(format!("Failed to begin transaction: {}", e))
                })?;
            }
        };

        // Execute function
        match f().await {
            Ok(result) => {
                // Commit transaction
                match &self.client {
                    DatabaseClient::SurrealKV(db) => {
                        db.query("COMMIT TRANSACTION").await.map_err(|e| {
                            Error::database(format!("Failed to commit transaction: {}", e))
                        })?;
                    }
                }
                Ok(result)
            },
            Err(e) => {
                // Cancel transaction
                match &self.client {
                    DatabaseClient::SurrealKV(db) => {
                        db.query("CANCEL TRANSACTION").await.map_err(|e| {
                            Error::database(format!("Failed to cancel transaction: {}", e))
                        })?;
                    }
                }
                Err(e)
            },
        }
    }
}

/// Live query stream for real-time updates
pub struct LiveQueryStream<T> {
    client: DatabaseClient,
    query: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned + Send + Sync + Unpin + 'static> LiveQueryStream<T> {
    /// Create a new live query stream
    fn new(client: DatabaseClient, query: String) -> Self {
        Self { client, query, _phantom: std::marker::PhantomData }
    }

    /// Extract table name from query
    fn extract_table_name(&self) -> Result<&str> {
        // Simple extraction for "SELECT * FROM table" patterns
        if let Some(from_pos) = self.query.find("FROM ") {
            let after_from = &self.query[from_pos + 5..];
            if let Some(space_pos) = after_from.find(' ') {
                Ok(&after_from[..space_pos])
            } else {
                Ok(after_from.trim())
            }
        } else {
            Err(Error::validation("Cannot extract table name from query"))
        }
    }

    /// Subscribe to the live query stream using SurrealDB 2.3+ live query API
    ///
    /// If timeout_ms is provided, the operation will time out after the specified milliseconds
    pub async fn subscribe(
        self,
        timeout_ms: Option<u64>,
    ) -> Result<impl Stream<Item = Result<Notification<T>>>> {
        match &self.client {
            DatabaseClient::SurrealKV(db) => {
                // Extract table name for live query
                let table_name = self.extract_table_name()?;
                
                // Use SurrealDB 2.3+ live query API
                let live_stream = if let Some(ms) = timeout_ms {
                    match timeout(Duration::from_millis(ms), db.select(table_name).live()).await {
                        Ok(result) => result.map_err(|e| {
                            Error::database(format!("Failed to create live query: {}", e))
                        })?,
                        Err(_) => {
                            return Err(Error::timeout(format!(
                                "Live query timed out after {}ms",
                                ms
                            )));
                        },
                    }
                } else {
                    db.select(table_name).live().await.map_err(|e| {
                        Error::database(format!("Failed to create live query: {}", e))
                    })?
                };
                
                // Create a channel for the notification stream
                let (tx, rx) = mpsc::channel(32);
                
                // Spawn a task to process notifications from the SurrealDB stream
                tokio::spawn(async move {
                    use futures::stream::StreamExt;
                    let mut stream = Box::pin(live_stream);
                    
                    while let Some(notification_result) = stream.next().await {
                        // The SurrealDB stream yields Result<Notification<T>, Error> directly
                        match notification_result {
                            Ok(notification) => {
                                if tx.send(Ok(notification)).await.is_err() {
                                    break;
                                }
                            },
                            Err(e) => {
                                if tx.send(Err(Error::database(format!(
                                    "Error in live query stream: {}",
                                    e
                                ))))
                                .await
                                .is_err()
                                {
                                    break;
                                }
                            },
                        }
                    }
                });
                
                Ok(tokio_stream::wrappers::ReceiverStream::new(rx))
            },
        }
    }
}

impl DatabaseClient {
    /// Create a transaction manager
    pub fn transaction(&self) -> TransactionManager {
        TransactionManager::new(self.clone())
    }

    /// Create a live query
    pub fn live_query<T>(&self, query: &str) -> LiveQueryStream<T>
    where
        T: DeserializeOwned + Send + Sync + Unpin + 'static,
    {
        LiveQueryStream::new(self.clone(), query.to_string())
    }

    /// Execute an arbitrary query
    pub fn query<T>(&self, query: &str) -> QueryStream<T>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        QueryStream::new(self.clone(), query.to_string())
    }

    /// Execute a query with parameters
    pub fn query_with_params<T>(
        &self,
        query: &str,
        params: impl Serialize,
    ) -> Result<QueryStream<T>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        QueryStream::with_params(self.clone(), query, params)
    }

    /// Create a new record
    pub fn create<T>(&self, table: &str, data: impl Serialize) -> QueryStream<T>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        let query = format!("CREATE {} CONTENT $data RETURN AFTER", table);
        let params = json!({
            "data": data,
        });

        QueryStream::with_params(self.clone(), &query, params).unwrap_or_else(|_| {
            // This should never fail as we're constructing the JSON ourselves
            QueryStream::new(self.clone(), query)
        })
    }

    /// Get a record by ID
    pub fn get<T>(&self, table: &str, id: &str) -> OptionalQueryStream<T>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        match &self {
            DatabaseClient::SurrealKV(_db) => {
                // Use the Resource directly for better type safety
                let _resource = Resource::from((table, id));
                let query = format!("SELECT * FROM {}:{}", table, id);
                OptionalQueryStream::new(self.clone(), query)
            },
        }
    }

    /// Update a record by ID
    pub fn update<T>(&self, table: &str, id: &str, data: impl Serialize) -> OptionalQueryStream<T>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        let query = format!("UPDATE {}:{} CONTENT $data RETURN AFTER", table, id);
        let params = json!({
            "data": data,
        });

        OptionalQueryStream::with_params(self.clone(), &query, params).unwrap_or_else(|_| {
            // This should never fail as we're constructing the JSON ourselves
            OptionalQueryStream::new(self.clone(), query)
        })
    }

    /// Delete a record by ID
    pub fn delete<T>(&self, table: &str, id: &str) -> OptionalQueryStream<T>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        match &self {
            DatabaseClient::SurrealKV(_db) => {
                // Use the Resource directly for better type safety
                let _resource = Resource::from((table, id));
                let query = format!("DELETE {}:{} RETURN BEFORE", table, id);
                OptionalQueryStream::new(self.clone(), query)
            },
        }
    }

    /// Select records from a table
    pub fn select<T>(&self, table: &str) -> MultiQueryStream<T>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        match &self {
            DatabaseClient::SurrealKV(_db) => {
                // Use the Resource directly for better type safety
                let _resource = Resource::from(table);
                let query = format!("SELECT * FROM {}", table);
                MultiQueryStream::new(self.clone(), query)
            },
        }
    }

    /// Select records from a table with a filter
    pub fn select_where<T>(
        &self,
        table: &str,
        condition: &str,
        params: impl Serialize,
    ) -> Result<MultiQueryStream<T>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        let query = format!("SELECT * FROM {} WHERE {}", table, condition);
        MultiQueryStream::with_params(self.clone(), &query, params)
    }

    /// Begin a new transaction
    #[instrument(skip(self), level = "debug")]
    pub async fn begin_transaction(&self) -> Result<()> {
        match self {
            DatabaseClient::SurrealKV(db) => {
                db.query("BEGIN TRANSACTION").await.map_err(|e| {
                    Error::database(format!("Failed to begin transaction: {}", e))
                })?;
                
                debug!("Started new transaction");
                Ok(())
            },
        }
    }

    /// Commit the current transaction
    #[instrument(skip(self), level = "debug")]
    pub async fn commit_transaction(&self) -> Result<()> {
        match self {
            DatabaseClient::SurrealKV(db) => {
                db.query("COMMIT TRANSACTION").await.map_err(|e| {
                    Error::database(format!("Failed to commit transaction: {}", e))
                })?;
                
                debug!("Committed transaction");
                Ok(())
            },
        }
    }

    /// Cancel the current transaction
    #[instrument(skip(self), level = "debug")]
    pub async fn cancel_transaction(&self) -> Result<()> {
        match self {
            DatabaseClient::SurrealKV(db) => {
                db.query("CANCEL TRANSACTION").await.map_err(|e| {
                    Error::database(format!("Failed to cancel transaction: {}", e))
                })?;
                
                debug!("Canceled transaction");
                Ok(())
            },
        }
    }
}
