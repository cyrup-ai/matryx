//! Matrix Client-Server API: Individual Device Operations
//!
//! Implementation of per-device endpoints per Matrix spec v1.8

use super::DeviceInfo;
use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request body for updating device metadata
#[derive(Debug, Serialize)]
struct UpdateDeviceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
}

/// Request body for deleting device (may include UIA auth)
#[derive(Debug, Serialize)]
struct DeleteDeviceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<Value>,
}

/// Empty response structure for successful operations
#[derive(Debug, Deserialize)]
struct EmptyResponse {}

/// Client for individual device operations
#[derive(Clone)]
pub struct DeviceByIdClient {
    http_client: MatrixHttpClient,
}

impl DeviceByIdClient {
    /// Create a new device-by-id client
    ///
    /// # Arguments
    /// * `http_client` - HTTP client configured with homeserver URL
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get information about a single device
    ///
    /// Endpoint: GET /_matrix/client/v3/devices/{deviceId}
    /// Authentication: Required
    ///
    /// # Arguments
    /// * `device_id` - The device to retrieve
    ///
    /// # Returns
    /// * `Result<DeviceInfo, HttpClientError>` - Device information or error
    ///
    /// # Errors
    /// * 404 if device doesn't exist for current user
    pub async fn get_device(&self, device_id: &str) -> Result<DeviceInfo, HttpClientError> {
        let path = format!("/_matrix/client/v3/devices/{}", device_id);
        
        self.http_client
            .request(Method::GET, &path, None::<&()>)
            .await
    }

    /// Update device metadata
    ///
    /// Endpoint: PUT /_matrix/client/v3/devices/{deviceId}
    /// Authentication: Required
    ///
    /// # Arguments
    /// * `device_id` - The device to update
    /// * `display_name` - New display name (if None, display name is unchanged)
    ///
    /// # Returns
    /// * `Result<(), HttpClientError>` - Success or error
    ///
    /// # Errors
    /// * 404 if device doesn't exist for current user
    pub async fn update_device(
        &self,
        device_id: &str,
        display_name: Option<String>,
    ) -> Result<(), HttpClientError> {
        let path = format!("/_matrix/client/v3/devices/{}", device_id);
        let body = UpdateDeviceRequest { display_name };

        let _: EmptyResponse = self
            .http_client
            .request(Method::PUT, &path, Some(&body))
            .await?;

        Ok(())
    }

    /// Delete a device, invalidating its access token
    ///
    /// Endpoint: DELETE /_matrix/client/v3/devices/{deviceId}
    /// Authentication: Required (may require UIA re-authentication)
    ///
    /// # Arguments
    /// * `device_id` - The device to delete
    /// * `auth` - Optional UIA auth object for re-authentication
    ///
    /// # Returns
    /// * `Result<(), HttpClientError>` - Success or error
    ///
    /// # Matrix Spec
    /// Device deletion may require User-Interactive Authentication.
    /// If UIA is required, the server will return a 401 with auth flows.
    pub async fn delete_device(
        &self,
        device_id: &str,
        auth: Option<Value>,
    ) -> Result<(), HttpClientError> {
        let path = format!("/_matrix/client/v3/devices/{}", device_id);
        let body = DeleteDeviceRequest { auth };

        let _: EmptyResponse = self
            .http_client
            .request(Method::DELETE, &path, Some(&body))
            .await?;

        Ok(())
    }
}
