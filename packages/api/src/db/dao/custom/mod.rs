use crate::db::client::DatabaseClient;
use crate::db::entity::custom_value::CustomValue;
use crate::db::error::Result;
use crate::db::generic_dao::{Dao, Entity};
use chrono::Utc;

/// DAO for custom values
#[derive(Clone)]
pub struct CustomDao {
    dao: Dao<CustomValue>,
}

impl CustomDao {
    const TABLE_NAME: &'static str = "custom_store";

    /// Create a new CustomDao
    pub fn new(client: DatabaseClient) -> Self {
        Self {
            dao: Dao::new(client, Self::TABLE_NAME),
        }
    }

    /// Get a value by key
    pub async fn get_value(&self, key: &str) -> Result<Option<CustomValue>> {
        self.dao.find_by_field("key", key).await
    }

    /// Set a value
    pub async fn set_value(&self, key: &str, value: Vec<u8>) -> Result<CustomValue> {
        // Check if the key already exists
        match self.get_value(key).await? {
            Some(mut existing) => {
                // Update existing value
                existing.value = value;
                existing.created_at = Utc::now();
                self.dao.update(&existing).await
            },
            None => {
                // Create new value
                let new_value = CustomValue {
                    key: key.to_string(),
                    value,
                    created_at: Utc::now(),
                };
                self.dao.create(&new_value).await
            },
        }
    }

    /// Remove a value
    pub async fn remove_value(&self, key: &str) -> Result<Option<CustomValue>> {
        match self.get_value(key).await? {
            Some(value) => {
                match value.id() {
                    Some(id) => self.dao.delete(id).await,
                    None => Ok(None),
                }
            },
            None => Ok(None),
        }
    }

    /// Create a CustomValue entry
    pub async fn create(&self, entry: &CustomValue) -> Result<CustomValue> {
        self.dao.create(entry).await
    }

    /// Update a CustomValue entry
    pub async fn update(&self, entry: &CustomValue) -> Result<CustomValue> {
        self.dao.update(entry).await
    }

    /// Delete a CustomValue entry
    pub async fn delete(&self, id: &str) -> Result<Option<CustomValue>> {
        self.dao.delete(id).await
    }
    
    /// Create the custom_value table if it doesn't exist
    pub async fn create_table(&self) -> Result<()> {
        self.dao.create_table().await
    }
}