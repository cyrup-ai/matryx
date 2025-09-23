use futures::StreamExt;
use matryx_entity::types::MatrixFilter;
use matryx_surrealdb::repository::filter::{FilterLiveUpdate, FilterRepository};
use std::time::Duration;

#[tokio::test]
async fn test_live_query_creation_and_streaming() {
    let test_db = setup_test_database().await;
    let filter_repo = FilterRepository::new(test_db.clone());
    let user_id = "@test:example.com";

    // Start live query stream
    let mut stream = filter_repo.subscribe_user(user_id.to_string());

    // Create filter in separate task
    let filter_repo_clone = filter_repo.clone();
    let test_filter = create_test_matrix_filter();
    let test_filter_clone = test_filter.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Err(e) = filter_repo_clone.create(&test_filter_clone, "filter_1").await {
            panic!("Failed to create filter: {:?}", e);
        }
    });

    // Verify stream receives the new filter
    let received = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;
    assert!(received.is_ok());

    if let Ok(Some(Ok(FilterLiveUpdate::Created(filter)))) = received {
        assert_eq!(filter.event_format, test_filter.event_format);
    } else {
        panic!("Expected filter creation notification");
    }
}

#[tokio::test]
async fn test_live_query_cleanup_and_lifecycle() {
    let test_db = setup_test_database().await;
    let filter_repo = FilterRepository::new(test_db.clone());
    let user_id = "@test:example.com";

    // Start subscription
    let _stream = filter_repo.subscribe_user(user_id.to_string());

    // Verify subscription is active
    let active = match filter_repo.get_active_subscriptions() {
        Ok(subscriptions) => subscriptions,
        Err(e) => panic!("Failed to get active subscriptions: {:?}", e),
    };
    assert!(active.contains(&user_id.to_string()));

    // Unsubscribe
    if let Err(e) = filter_repo.unsubscribe_user(user_id).await {
        panic!("Failed to unsubscribe user: {:?}", e);
    }

    // Verify cleanup
    let active = match filter_repo.get_active_subscriptions() {
        Ok(subscriptions) => subscriptions,
        Err(e) => panic!("Failed to get active subscriptions after cleanup: {:?}", e),
    };
    assert!(!active.contains(&user_id.to_string()));
}

#[tokio::test]
async fn test_live_query_error_recovery() {
    // Test that live queries recover from database connection errors
    // Test exponential backoff behavior
    // Test max retry limits
    // Test graceful degradation
}

#[tokio::test]
async fn test_concurrent_live_queries() {
    // Test multiple users with concurrent live queries
    // Test resource cleanup under load
    // Test notification delivery accuracy
}

// Helper functions for testing
async fn setup_test_database() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    use surrealdb::engine::any;
    let db = any::connect("memory").await.expect("Failed to connect to memory database");
    db.use_ns("test")
        .use_db("test")
        .await
        .expect("Failed to set namespace/database");
    db
}

fn create_test_matrix_filter() -> MatrixFilter {
    use matryx_entity::types::MatrixFilter;
    MatrixFilter {
        event_fields: Some(vec!["content.body".to_string(), "sender".to_string()]),
        event_format: "client".to_string(),
        presence: None,
        account_data: None,
        room: None,
    }
}
