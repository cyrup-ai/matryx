use serde::{Deserialize, Serialize};
use std::path::Path;
use surrealdb_client::{
    connect_database, open_surrealkv_store, DbConfig, StorageEngine, SurrealKvStore,
};
use surrealkv::Options as SurrealKvOptions;
use tokio_test::block_on;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestData {
    name: String,
    value: i32,
}

#[test]
fn test_surrealkv_low_level_api() {
    // Create a temporary directory for testing
    let temp_dir = std::env::temp_dir().join("surrealkv_test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Open a SurrealKV store
    let store = block_on(async {
        let store = open_surrealkv_store(&temp_dir).unwrap();

        // Begin a transaction
        let mut txn = store.begin().unwrap();

        // Store some data
        let test_data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        txn.set_json(b"test_key", &test_data).unwrap();

        // Commit the transaction
        txn.commit().unwrap();

        // Test reading the data back
        let mut txn = store.begin().unwrap();
        let result: Option<TestData> = txn.get_json(b"test_key").unwrap();

        assert!(result.is_some());
        let data = result.unwrap();
        assert_eq!(data.name, "test");
        assert_eq!(data.value, 42);

        store
    });

    // Close the store
    block_on(async {
        store.close().unwrap();
    });

    // Clean up
    std::fs::remove_dir_all(temp_dir).unwrap_or_default();
}

#[tokio::test]
async fn test_surrealkv_storage_engine() {
    // Create a temporary directory for testing
    let temp_dir = std::env::temp_dir().join("surrealkv_engine_test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Set up SurrealKV config
    let config = DbConfig {
        engine: StorageEngine::SurrealKv,
        path: temp_dir.to_string_lossy().to_string(),
        namespace: "test".into(),
        database: "test".into(),
        run_migrations: false,
        ..Default::default()
    };

    // Connect to the database
    let client = connect_database(config).await.unwrap();

    // Create a simple test table
    let result = client.query("DEFINE TABLE kv_test").await.unwrap();

    // Insert a test record
    let data = serde_json::json!({
        "name": "test",
        "value": 42
    });

    let _created: serde_json::Value = client.create("kv_test", data).await.unwrap();

    // Query the record back
    let result: Vec<serde_json::Value> = client.select("kv_test").await.unwrap();

    // Verify we have one record
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["name"], "test");
    assert_eq!(result[0]["value"], 42);

    // Clean up
    std::fs::remove_dir_all(temp_dir).unwrap_or_default();
}
