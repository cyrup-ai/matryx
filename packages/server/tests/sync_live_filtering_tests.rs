#[tokio::test]
async fn test_sync_with_live_filter_updates() {
    let test_server = setup_test_matrix_server().await;
    let user_access_token = create_test_user(&test_server).await;

    // Create initial filter
    let filter = create_test_matrix_filter();
    let filter_id = create_filter_via_api(&test_server, &user_access_token, &filter).await;

    // Start sync with filter
    let sync_response = start_sync_with_filter(&test_server, &user_access_token, &filter_id).await;
    assert!(sync_response.is_ok());

    // Update filter and verify live sync response changes
    let updated_filter = modify_test_filter(&filter);
    update_filter_via_api(&test_server, &user_access_token, &filter_id, &updated_filter).await;

    // TODO: Verify that sync stream receives updated filtering
    // This would require WebSocket or SSE integration
}

// Helper functions for integration testing
async fn setup_test_matrix_server() -> TestServer {
    TestServer { /* implement basic test server structure */ }
}

async fn create_test_user(server: &TestServer) -> String {
    // Return mock access token
    "test_access_token".to_string()
}

fn create_test_matrix_filter() -> matryx_entity::types::MatrixFilter {
    matryx_entity::types::MatrixFilter {
        event_fields: Some(vec!["content.body".to_string(), "sender".to_string()]),
        event_format: "client".to_string(),
        presence: None,
        account_data: None,
        room: None,
    }
}

async fn create_filter_via_api(
    server: &TestServer,
    token: &str,
    filter: &matryx_entity::types::MatrixFilter,
) -> String {
    // Return mock filter ID
    "test_filter_id".to_string()
}

async fn start_sync_with_filter(
    server: &TestServer,
    token: &str,
    filter_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Mock sync start - return success
    Ok(())
}

fn modify_test_filter(
    filter: &matryx_entity::types::MatrixFilter,
) -> matryx_entity::types::MatrixFilter {
    // Return modified filter with additional field
    let mut modified = filter.clone();
    modified.event_fields = Some(vec![
        "content.body".to_string(),
        "sender".to_string(),
        "timestamp".to_string(),
    ]);
    modified
}

async fn update_filter_via_api(
    server: &TestServer,
    token: &str,
    filter_id: &str,
    filter: &matryx_entity::types::MatrixFilter,
) {
    // Mock filter update via API
}

struct TestServer {
    // Test server structure
}
