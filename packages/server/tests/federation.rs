mod integration;
mod test_config;

use integration::federation::*;
use test_config::TestConfig;

#[tokio::test]
async fn test_federation_endpoints() {
    let config = TestConfig::from_env();
    
    if !config.enable_federation_tests {
        println!("Federation tests disabled by configuration");
        return;
    }
    
    println!("Running federation endpoint tests");
    
    let federation_test = FederationTest::new().await
        .expect("Should be able to create federation test");
    
    // Test server discovery
    let result = federation_test.test_server_discovery().await;
    assert!(result.is_ok(), "Server discovery test failed: {:?}", result);
    
    // Test federation authentication
    let result = federation_test.test_federation_authentication().await;
    assert!(result.is_ok(), "Federation authentication test failed: {:?}", result);
    
    // Test cross-server operations
    let result = federation_test.test_cross_server_messaging().await;
    assert!(result.is_ok(), "Cross-server messaging test failed: {:?}", result);
    
    println!("✅ Federation endpoint tests completed");
}

#[tokio::test]
async fn test_federation_event_handling() {
    let config = TestConfig::from_env();
    
    if !config.enable_federation_tests {
        println!("Federation tests disabled by configuration");
        return;
    }
    
    let event_test = FederationEventTest::new().await
        .expect("Should be able to create federation event test");
    
    // Test federation event handling
    let result = event_test.test_federation_event_handling().await;
    assert!(result.is_ok(), "Federation event handling test failed: {:?}", result);
    
    // Test server keys
    let result = event_test.test_server_keys().await;
    assert!(result.is_ok(), "Server key test failed: {:?}", result);
    
    println!("✅ Federation event handling tests completed");
}

#[tokio::test]
async fn test_multi_server_federation() {
    let config = TestConfig::from_env();
    
    if !config.enable_federation_tests {
        println!("Federation tests disabled by configuration");
        return;
    }
    
    println!("Testing multi-server federation setup");
    
    let federation_test = FederationTest::new().await
        .expect("Should be able to create federation test");
    
    // Test that both servers are accessible
    let response1 = federation_test.server1.test_endpoint("GET", "/_matrix/client/versions", None).await;
    let response2 = federation_test.server2.test_endpoint("GET", "/_matrix/client/versions", None).await;
    
    assert_eq!(response1.status_code(), 200, "Server 1 should be accessible");
    assert_eq!(response2.status_code(), 200, "Server 2 should be accessible");
    
    println!("Both federation test servers are operational");
    
    // Test federation-specific endpoints exist
    let fed_response1 = federation_test.server1.test_endpoint("GET", "/_matrix/federation/v1/version", None).await;
    let fed_response2 = federation_test.server2.test_endpoint("GET", "/_matrix/federation/v1/version", None).await;
    
    // Federation endpoints should exist (may require auth, but should not 404)
    assert_ne!(fed_response1.status_code(), 404, "Server 1 federation endpoint should exist");
    assert_ne!(fed_response2.status_code(), 404, "Server 2 federation endpoint should exist");
    
    println!("✅ Multi-server federation test completed");
}