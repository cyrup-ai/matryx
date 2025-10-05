//! Matrix Client-Server API: Device Management
//!
//! Implementation of device management endpoints per Matrix spec v1.8
//! Reference: https://spec.matrix.org/v1.8/client-server-api/#device-management

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};

pub mod by_device_id;

/// Device information structure
///
/// Represents a Matrix device with metadata as defined in the spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Unique identifier of this device
    pub device_id: String,
    
    /// Display name set by the user for this device
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    
    /// IP address where this device was last seen
    /// May be a few minutes out of date for efficiency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ip: Option<String>,
    
    /// Timestamp (milliseconds since unix epoch) when device was last seen
    /// May be a few minutes out of date for efficiency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ts: Option<i64>,
}

/// Response structure for GET /_matrix/client/v3/devices
#[derive(Debug, Deserialize)]
struct GetDevicesResponse {
    devices: Vec<DeviceInfo>,
}

/// Client for device management operations
#[derive(Clone)]
pub struct DeviceClient {
    http_client: MatrixHttpClient,
}

impl DeviceClient {
    /// Create a new device client
    ///
    /// # Arguments
    /// * `http_client` - HTTP client configured with homeserver URL
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get all devices for the current user
    ///
    /// Endpoint: GET /_matrix/client/v3/devices
    /// Authentication: Required
    ///
    /// # Returns
    /// * `Result<Vec<DeviceInfo>, HttpClientError>` - List of devices or error
    ///
    /// # Matrix Spec
    /// Gets information about all devices for the current user
    pub async fn get_devices(&self) -> Result<Vec<DeviceInfo>, HttpClientError> {
        let response: GetDevicesResponse = self
            .http_client
            .request(
                Method::GET,
                "/_matrix/client/v3/devices",
                None::<&()>,
            )
            .await?;

        Ok(response.devices)
    }
}
