use crate::db::client::DatabaseClient;
use crate::db::entity::key_value::KeyValue;
use crate::db::error::Result;
use crate::db::generic_dao::Dao;

/// DAO for key-value pairs
#[derive(Clone)]
pub struct KeyValueDao {
    dao: Dao<KeyValue>,
}

impl KeyValueDao {
    const TABLE_NAME: &'static str = "key_value";

    /// Create a new KeyValueDao
    pub fn new(client: DatabaseClient) -> Self {
        Self {
            dao: Dao::new(client, Self::TABLE_NAME),
        }
    }

    /// Get a value by key
    pub async fn get_value(&self, key: &str) -> Result<Option<KeyValue>> {
        self.dao.find_by_field("key", key).await
    }

    /// Set a value
    pub async fn set_value(&self, entry: KeyValue) -> Result<KeyValue> {
        // Check if the key already exists
        match self.get_value(&entry.key).await? {
            Some(mut existing) => {
                // Update existing value with new type and value
                existing.value = entry.value;
                existing.value_type = entry.value_type;
                self.dao.update(&existing).await
            },
            None => {
                // Create new entry
                self.dao.create(&entry).await
            },
        }
    }

    /// Remove a value
    pub async fn remove_value(&self, key: &str) -> Result<Option<KeyValue>> {
        match self.get_value(key).await? {
            Some(value) => {
                match value.id {
                    Some(id) => self.dao.delete(id).await,
                    None => Ok(None),
                }
            },
            None => Ok(None),
        }
    }

    /// Create a KeyValue entry
    pub async fn create(&self, entry: &KeyValue) -> Result<KeyValue> {
        self.dao.create(entry).await
    }

    /// Update a KeyValue entry
    pub async fn update(&self, entry: &KeyValue) -> Result<KeyValue> {
        self.dao.update(entry).await
    }

    /// Delete a KeyValue entry
    pub async fn delete(&self, id: &str) -> Result<Option<KeyValue>> {
        self.dao.delete(id).await
    }

    /// Create the key_value table if it doesn't exist
    pub async fn create_table(&self) -> Result<()> {
        self.dao.create_table().await
    }
}