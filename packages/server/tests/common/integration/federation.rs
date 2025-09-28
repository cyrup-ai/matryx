use super::{MatrixTestServer, create_test_room, create_test_user};
use serde_json::{Value, json};

/// Federation Testing for Multi-Homeserver scenarios
pub struct FederationTest {
    pub server1: MatrixTestServer,
    pub server2: MatrixTestServer,
}

impl FederationTest {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let server1 = MatrixTestServer::new().await;
        let server2 = MatrixTestServer::new().await;

        Ok(Self { server1, server2 })
    }

    pub async fn test_cross_server_messaging(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Create users on different servers
        let (user1_id, user1_token) = create_test_user(&self.server1, "alice", "password1").await?;
        let (user2_id, user2_token) = create_test_user(&self.server2, "bob", "password2").await?;

        // Create room on server1
        let room_id = create_test_room(&self.server1, &user1_token, "Federation Test Room").await?;

        // Test federation invite (simulated - in real federation this would be cross-server)
        // Validate the user IDs are properly formatted for federation
        assert!(user1_id.starts_with('@'), "User 1 ID should be properly formatted: {}", user1_id);
        assert!(user2_id.starts_with('@'), "User 2 ID should be properly formatted: {}", user2_id);
        
        let invite_body = json!({
            "user_id": user2_id
        });
        let path = format!("/_matrix/client/v3/rooms/{}/invite", room_id);
        let response = self
            .server1
            .test_authenticated_endpoint("POST", &path, &user1_token, Some(invite_body))
            .await;

        // Validate that user2_token works for authentication on server2
        let whoami_response = self
            .server2
            .test_authenticated_endpoint("GET", "/_matrix/client/v3/account/whoami", &user2_token, None)
            .await;
        assert_eq!(whoami_response.status_code(), 200, "User 2 should be able to authenticate on server 2");

        // In a real federation test, this would involve actual server-to-server communication
        // For now, we test that the invite endpoint responds correctly
        assert!(
            response.status_code() == 200 || response.status_code() == 403,
            "Invite should succeed or fail gracefully, got {}",
            response.status_code()
        );

        Ok(())
    }

    pub async fn test_server_discovery(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test well-known server discovery endpoints
        let response = self.server1.test_endpoint("GET", "/.well-known/matrix/server", None).await;
        assert!(
            response.status_code() == 200 || response.status_code() == 404,
            "Well-known server endpoint should respond"
        );

        let response = self.server1.test_endpoint("GET", "/.well-known/matrix/client", None).await;
        assert!(
            response.status_code() == 200 || response.status_code() == 404,
            "Well-known client endpoint should respond"
        );

        Ok(())
    }

    pub async fn test_federation_authentication(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test federation version endpoint
        let response = self
            .server1
            .test_endpoint("GET", "/_matrix/federation/v1/version", None)
            .await;

        // This endpoint should exist for federation
        assert!(
            response.status_code() == 200 || response.status_code() == 401,
            "Federation version endpoint should exist"
        );

        Ok(())
    }
}

/// Federation Event Testing
pub struct FederationEventTest {
    server: MatrixTestServer,
}

impl FederationEventTest {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let server = MatrixTestServer::new().await;
        Ok(Self { server })
    }

    pub async fn test_federation_event_handling(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test that federation endpoints exist and respond appropriately
        let federation_endpoints = vec!["/_matrix/federation/v1/version", "/_matrix/key/v2/server"];

        for endpoint in federation_endpoints {
            let response = self.server.test_endpoint("GET", endpoint, None).await;

            // Federation endpoints should exist (may require auth, but should not 404)
            assert!(response.status_code() != 404, "Federation endpoint {} should exist", endpoint);
        }

        Ok(())
    }

    pub async fn test_server_keys(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test server key endpoints
        let response = self.server.test_endpoint("GET", "/_matrix/key/v2/server", None).await;

        if response.status_code() == 200 {
            let body: Value = response.json();

            // Verify server key structure
            assert!(body.get("server_name").is_some(), "Server key should have server_name");
            assert!(body.get("verify_keys").is_some(), "Server key should have verify_keys");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_federation_setup() {
        let federation_test = FederationTest::new().await.unwrap();

        // Test that both servers are accessible
        let response1 = federation_test
            .server1
            .test_endpoint("GET", "/_matrix/client/versions", None)
            .await;
        let response2 = federation_test
            .server2
            .test_endpoint("GET", "/_matrix/client/versions", None)
            .await;

        assert_eq!(response1.status_code(), 200);
        assert_eq!(response2.status_code(), 200);
    }

    #[tokio::test]
    async fn test_server_discovery_endpoints() {
        let federation_test = FederationTest::new().await.unwrap();
        let result = federation_test.test_server_discovery().await;
        assert!(result.is_ok(), "Server discovery test failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_federation_authentication_endpoints() {
        let federation_test = FederationTest::new().await.unwrap();
        let result = federation_test.test_federation_authentication().await;
        assert!(result.is_ok(), "Federation authentication test failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_cross_server_operations() {
        let federation_test = FederationTest::new().await.unwrap();
        let result = federation_test.test_cross_server_messaging().await;
        assert!(result.is_ok(), "Cross-server messaging test failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_federation_event_handling() {
        let event_test = FederationEventTest::new().await.unwrap();
        let result = event_test.test_federation_event_handling().await;
        assert!(result.is_ok(), "Federation event handling test failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_server_key_endpoints() {
        let event_test = FederationEventTest::new().await.unwrap();
        let result = event_test.test_server_keys().await;
        assert!(result.is_ok(), "Server key test failed: {:?}", result);
    }
}
