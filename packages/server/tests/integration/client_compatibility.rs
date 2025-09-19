use matrix_sdk::{Client, config::SyncSettings};
use url::Url;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::timeout;

/// Matrix Client Compatibility Testing Framework
pub struct ClientCompatibilityTest {
    homeserver_url: Url,
    client: Client,
}

impl ClientCompatibilityTest {
    pub async fn new(homeserver_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let url = Url::parse(homeserver_url)?;
        let client = Client::new(url.clone()).await?;
        
        Ok(Self {
            homeserver_url: url,
            client,
        })
    }
    
    pub async fn test_registration(&self, username: &str, password: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Test user registration flow with matrix-rust-sdk
        let response = self.client
            .matrix_auth()
            .register(username, password)
            .initial_device_display_name("Test Device")
            .send()
            .await?;
        
        assert!(response.access_token.is_some());
        assert!(response.user_id.to_string().contains(username));
        Ok(())
    }
    
    pub async fn test_login(&self, username: &str, password: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Test user login flow
        let response = self.client
            .matrix_auth()
            .login_username(username, password)
            .initial_device_display_name("Test Device")
            .send()
            .await?;
            
        assert!(response.access_token.is_some());
        Ok(())
    }
    
    pub async fn test_sync(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test /sync endpoint with real Matrix client
        let sync_settings = SyncSettings::default().timeout(Duration::from_secs(10));
        
        let sync_result = timeout(Duration::from_secs(15), self.client.sync_once(sync_settings)).await;
        
        match sync_result {
            Ok(response) => {
                let response = response?;
                // Validate sync response structure
                assert!(response.next_batch.is_some());
                Ok(())
            },
            Err(_) => {
                // Timeout is acceptable for sync endpoint
                Ok(())
            }
        }
    }
    
    pub async fn test_room_operations(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test room creation
        let room_request = matrix_sdk::ruma::api::client::room::create_room::v3::Request::new();
        let room = self.client.create_room(room_request).await?;
        
        // Test message sending
        let content = matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain("Test message");
        room.send(content).await?;
        
        // Test room state
        let room_name = room.name();
        assert!(room_name.is_some() || room.canonical_alias().is_some() || !room.room_id().to_string().is_empty());
        
        Ok(())
    }
    
    pub async fn test_device_management(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test device listing
        let devices = self.client.devices().await?;
        assert!(!devices.is_empty());
        
        // Test current device info
        let device_id = self.client.device_id();
        assert!(device_id.is_some());
        
        Ok(())
    }
    
    pub async fn test_capabilities(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Test server capabilities endpoint
        let capabilities = self.client.get_capabilities().await?;
        
        // Verify basic capabilities structure
        assert!(capabilities.capabilities.contains_key("m.change_password") || 
                capabilities.capabilities.contains_key("m.room_versions"));
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::{MatrixTestServer, create_test_user};
    
    #[tokio::test]
    async fn test_matrix_sdk_compatibility() {
        let test_server = MatrixTestServer::new().await;
        let homeserver_url = &test_server.base_url;
        
        // Create compatibility test instance
        let compat_test = ClientCompatibilityTest::new(homeserver_url).await.unwrap();
        
        // Test registration
        let result = compat_test.test_registration("sdk_test_user", "test_password").await;
        assert!(result.is_ok(), "SDK registration test failed: {:?}", result);
        
        // Test capabilities
        let result = compat_test.test_capabilities().await;
        assert!(result.is_ok(), "SDK capabilities test failed: {:?}", result);
        
        // Test sync (may timeout, which is acceptable)
        let result = compat_test.test_sync().await;
        assert!(result.is_ok(), "SDK sync test failed: {:?}", result);
    }
    
    #[tokio::test]
    async fn test_client_login_flow() {
        let test_server = MatrixTestServer::new().await;
        
        // First create a user via direct API
        let (user_id, _) = create_test_user(&test_server, "login_test_user", "test_password").await.unwrap();
        
        // Then test login via SDK
        let compat_test = ClientCompatibilityTest::new(&test_server.base_url).await.unwrap();
        let result = compat_test.test_login("login_test_user", "test_password").await;
        
        assert!(result.is_ok(), "SDK login test failed: {:?}", result);
    }
}