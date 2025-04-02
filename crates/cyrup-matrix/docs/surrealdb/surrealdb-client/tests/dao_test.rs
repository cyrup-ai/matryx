use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb_client::{
    client::{connect_database, DatabaseClient},
    config::{DbConfig, StorageEngine},
    dao::{BaseDao, BaseEntity, Dao, Entity},
    error::Result,
};
use tokio::sync::Mutex;

// Define a test entity
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestEntity {
    #[serde(flatten)]
    base: BaseEntity,
    name: String,
    value: i32,
}

impl Entity for TestEntity {
    fn table_name() -> &'static str {
        "test_entities"
    }

    fn id(&self) -> Option<String> {
        self.base.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.base.id = Some(id);
    }
}

impl TestEntity {
    fn new(name: impl Into<String>, value: i32) -> Self {
        Self {
            base: BaseEntity::new(),
            name: name.into(),
            value,
        }
    }
}

// Test the DAO implementation with the Hidden Box Pin pattern
#[tokio::test]
async fn test_dao_crud_operations() -> Result<()> {
    // Create an in-memory database for testing
    let config = DbConfig {
        engine: StorageEngine::Memory,
        namespace: "test".into(),
        database: "test".into(),
        ..Default::default()
    };

    let client = connect_database(config)?;

    // Create DAO
    let dao = Dao::<TestEntity>::new(client);

    // Create a table for the test entity
    dao.create_table().success().await?;

    // Create a test entity
    let mut entity = TestEntity::new("test", 42);
    let created = dao.create(&mut entity).entity().await?;

    // Verify the entity was created with an ID
    assert!(created.id().is_some());
    assert_eq!(created.name, "test");
    assert_eq!(created.value, 42);

    // Get the entity by ID
    let id = created.id().unwrap();
    let retrieved = dao.get(&id).optional_entity().await?;

    // Verify we got the same entity
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id(), Some(id.clone()));
    assert_eq!(retrieved.name, "test");
    assert_eq!(retrieved.value, 42);

    // Update the entity
    let mut to_update = retrieved.clone();
    to_update.name = "updated".into();
    to_update.value = 99;

    let updated = dao.update(&to_update).optional_entity().await?;

    // Verify the entity was updated
    assert!(updated.is_some());
    let updated = updated.unwrap();
    assert_eq!(updated.id(), Some(id.clone()));
    assert_eq!(updated.name, "updated");
    assert_eq!(updated.value, 99);

    // Get all entities
    let all = dao.get_all().entities().await?;

    // Verify we have just one entity
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id(), Some(id.clone()));

    // Delete the entity
    let deleted = dao.delete(&id).optional_entity().await?;

    // Verify the entity was deleted
    assert!(deleted.is_some());
    let deleted = deleted.unwrap();
    assert_eq!(deleted.id(), Some(id.clone()));

    // Try to get the deleted entity
    let not_found = dao.get(&id).optional_entity().await?;

    // Verify the entity is gone
    assert!(not_found.is_none());

    Ok(())
}

// Test concurrent operations with the Hidden Box Pin pattern
#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    // Create an in-memory database for testing
    let config = DbConfig {
        engine: StorageEngine::Memory,
        namespace: "test_concurrent".into(),
        database: "test_concurrent".into(),
        ..Default::default()
    };

    let client = connect_database(config)?;

    // Create DAO
    let dao = Arc::new(Dao::<TestEntity>::new(client));

    // Create a table for the test entity
    dao.create_table().success().await?;

    // Spawn multiple tasks that create entities concurrently
    let mut handles = Vec::new();
    for i in 0..10 {
        let dao = dao.clone();
        let handle = tokio::spawn(async move {
            let mut entity = TestEntity::new(format!("entity_{}", i), i);
            let created = dao.create(&mut entity).entity().await.unwrap();
            created.id().unwrap()
        });
        handles.push(handle);
    }

    // Wait for all entities to be created
    let mut ids = Vec::new();
    for handle in handles {
        ids.push(handle.await.unwrap());
    }

    // Get all entities
    let all = dao.get_all().entities().await?;

    // Verify we have 10 entities
    assert_eq!(all.len(), 10);

    // Spawn multiple tasks that update entities concurrently
    let mut handles = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let dao = dao.clone();
        let id = id.clone();
        let handle = tokio::spawn(async move {
            let entity = dao.get(&id).optional_entity().await.unwrap().unwrap();
            let mut to_update = entity.clone();
            to_update.value = 100 + i as i32;
            let updated = dao.update(&to_update).optional_entity().await.unwrap();
            updated.unwrap().value
        });
        handles.push(handle);
    }

    // Wait for all entities to be updated
    for (i, handle) in handles.iter().enumerate() {
        let value = handle.await.unwrap();
        assert_eq!(value, 100 + i as i32);
    }

    // Delete all entities concurrently
    let mut handles = Vec::new();
    for id in ids {
        let dao = dao.clone();
        let handle = tokio::spawn(async move {
            dao.delete(&id).optional_entity().await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all entities to be deleted
    for handle in handles {
        handle.await.unwrap();
    }

    // Get all entities again
    let all = dao.get_all().entities().await?;

    // Verify all entities were deleted
    assert_eq!(all.len(), 0);

    Ok(())
}
