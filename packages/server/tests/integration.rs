mod integration;
mod test_config;

use integration::*;
use test_config::TestConfig;

#[tokio::test]
async fn test_matrix_server_integration() {
    let config = TestConfig::from_env();
    let test_server = MatrixTestServer::new().await;
    
    println!("Testing Matrix homeserver integration with config: {:?}", config.homeserver_name);
    
    // Test basic server functionality
    let response = test_server.test_endpoint("GET", "/_matrix/client/versions", None).await;
    assert_eq!(response.status_code(), 200, "Client versions endpoint should work");
    
    // Test user registration
    let (user_id, access_token) = create_test_user(&test_server, "integration_test_user", "test_password")
        .await
        .expect("Should be able to create test user");
    
    println!("Created test user: {}", user_id);
    
    // Test authenticated endpoint
    let response = test_server.test_authenticated_endpoint("GET", "/_matrix/client/v3/account/whoami", &access_token, None).await;
    assert_eq!(response.status_code(), 200, "Whoami endpoint should work with valid token");
    
    // Test room creation
    let room_id = create_test_room(&test_server, &access_token, "Integration Test Room")
        .await
        .expect("Should be able to create test room");
    
    println!("Created test room: {}", room_id);
    
    println!("✅ Matrix server integration test passed");
}