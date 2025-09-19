//! Device management for Matrix client
//!
//! Handles device registration, key management, and device-to-device communication
//! required for end-to-end encryption.

use anyhow::Result;
use matryx_entity::{Device, DeviceKeys};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::MatrixClient;

/// Device information for the current client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDevice {
    /// Device ID assigned by the server
    pub device_id: String,
    /// Display name for this device
    pub display_name: Option<String>,
    /// Device keys for encryption
    pub keys: Option<DeviceKeys>,
    /// One-time keys for key exchange
    pub one_time_keys: Option<HashMap<String, serde_json::Value>>,
}

/// Device management functionality
impl MatrixClient {
    /// Get information about the current device
    pub async fn get_device(&self, device_id: &str) -> Result<Device> {
        let path = format!("/_matrix/client/v3/devices/{}", device_id);
        let request = self.authenticated_request(Method::GET, &path)?;
        let response = request.send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to get device: {}", error_text));
        }

        let device: Device = response.json().await?;
        Ok(device)
    }

    /// Get all devices for the current user
    pub async fn get_devices(&self) -> Result<Vec<Device>> {
        let request = self.authenticated_request(Method::GET, "/_matrix/client/v3/devices")?;
        let response = request.send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to get devices: {}", error_text));
        }

        #[derive(Deserialize)]
        struct DevicesResponse {
            devices: Vec<Device>,
        }

        let devices_response: DevicesResponse = response.json().await?;
        Ok(devices_response.devices)
    }

    /// Update device information
    pub async fn update_device(&self, device_id: &str, display_name: Option<&str>) -> Result<()> {
        let path = format!("/_matrix/client/v3/devices/{}", device_id);

        let mut update_data = serde_json::json!({});
        if let Some(name) = display_name {
            update_data["display_name"] = serde_json::Value::String(name.to_string());
        }

        let request = self.authenticated_request(Method::PUT, &path)?;
        let response = request.json(&update_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to update device: {}", error_text));
        }

        Ok(())
    }

    /// Delete a device (requires authentication)
    pub async fn delete_device(&self, device_id: &str, auth_data: serde_json::Value) -> Result<()> {
        let path = format!("/_matrix/client/v3/devices/{}", device_id);

        let delete_data = serde_json::json!({
            "auth": auth_data
        });

        let request = self.authenticated_request(Method::DELETE, &path)?;
        let response = request.json(&delete_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to delete device: {}", error_text));
        }

        Ok(())
    }

    /// Delete multiple devices (requires authentication)
    pub async fn delete_devices(
        &self,
        device_ids: &[String],
        auth_data: serde_json::Value,
    ) -> Result<()> {
        let delete_data = serde_json::json!({
            "devices": device_ids,
            "auth": auth_data
        });

        let request =
            self.authenticated_request(Method::POST, "/_matrix/client/v3/delete_devices")?;
        let response = request.json(&delete_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to delete devices: {}", error_text));
        }

        Ok(())
    }

    /// Send a message to specific devices
    pub async fn send_to_device(
        &self,
        event_type: &str,
        messages: HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Result<()> {
        let txn_id = uuid::Uuid::new_v4().to_string();
        let path = format!("/_matrix/client/v3/sendToDevice/{}/{}", event_type, txn_id);

        let send_data = serde_json::json!({
            "messages": messages
        });

        let request = self.authenticated_request(Method::PUT, &path)?;
        let response = request.json(&send_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to send to device: {}", error_text));
        }

        Ok(())
    }

    /// Upload device keys for end-to-end encryption
    pub async fn upload_keys(
        &self,
        device_keys: Option<DeviceKeys>,
        one_time_keys: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<UploadKeysResponse> {
        let mut upload_data = serde_json::json!({});

        if let Some(keys) = device_keys {
            upload_data["device_keys"] = serde_json::to_value(keys)?;
        }

        if let Some(otk) = one_time_keys {
            upload_data["one_time_keys"] = serde_json::to_value(otk)?;
        }

        let request = self.authenticated_request(Method::POST, "/_matrix/client/v3/keys/upload")?;
        let response = request.json(&upload_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to upload keys: {}", error_text));
        }

        let upload_response: UploadKeysResponse = response.json().await?;
        Ok(upload_response)
    }

    /// Query device keys for users
    pub async fn query_keys(
        &self,
        device_keys: HashMap<String, Vec<String>>,
    ) -> Result<QueryKeysResponse> {
        let query_data = serde_json::json!({
            "device_keys": device_keys
        });

        let request = self.authenticated_request(Method::POST, "/_matrix/client/v3/keys/query")?;
        let response = request.json(&query_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to query keys: {}", error_text));
        }

        let query_response: QueryKeysResponse = response.json().await?;
        Ok(query_response)
    }

    /// Claim one-time keys for users
    pub async fn claim_keys(
        &self,
        one_time_keys: HashMap<String, HashMap<String, String>>,
    ) -> Result<ClaimKeysResponse> {
        let claim_data = serde_json::json!({
            "one_time_keys": one_time_keys
        });

        let request = self.authenticated_request(Method::POST, "/_matrix/client/v3/keys/claim")?;
        let response = request.json(&claim_data).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to claim keys: {}", error_text));
        }

        let claim_response: ClaimKeysResponse = response.json().await?;
        Ok(claim_response)
    }
}

/// Response from uploading keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadKeysResponse {
    /// Count of one-time keys by algorithm
    pub one_time_key_counts: HashMap<String, u64>,
}

/// Response from querying keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryKeysResponse {
    /// Device keys for each user
    pub device_keys: HashMap<String, HashMap<String, DeviceKeys>>,
    /// Master keys for cross-signing
    pub master_keys: Option<HashMap<String, serde_json::Value>>,
    /// Self-signing keys for cross-signing
    pub self_signing_keys: Option<HashMap<String, serde_json::Value>>,
    /// User-signing keys for cross-signing
    pub user_signing_keys: Option<HashMap<String, serde_json::Value>>,
}

/// Response from claiming keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimKeysResponse {
    /// One-time keys for each user and device
    pub one_time_keys: HashMap<String, HashMap<String, HashMap<String, serde_json::Value>>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClientConfig;

    #[tokio::test]
    async fn test_device_management_creation() {
        let config = ClientConfig::default();
        let client = MatrixClient::new(config).unwrap();

        // Test that device management methods exist and compile
        assert!(!client.is_authenticated());
    }

    #[test]
    fn test_device_structs() {
        let device = ClientDevice {
            device_id: "DEVICE123".to_string(),
            display_name: Some("Test Device".to_string()),
            keys: None,
            one_time_keys: None,
        };

        assert_eq!(device.device_id, "DEVICE123");
        assert_eq!(device.display_name.as_deref(), Some("Test Device"));
    }
}
