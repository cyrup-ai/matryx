//! Matrix Client-Server API: Delete Multiple Devices
//!
//! Implementation of bulk device deletion with UIA per Matrix spec v1.8
//! Reference: https://spec.matrix.org/v1.8/client-server-api/#device-management

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// User-Interactive Authentication data
/// Used for operations requiring additional authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthData {
    /// Authentication type (e.g., "m.login.password")
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<String>,
    
    /// Session identifier from homeserver
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    
    /// User identifier for password auth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    
    /// Password for password auth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    
    /// Other auth-specific fields
    #[serde(flatten)]
    pub additional: serde_json::Map<String, serde_json::Value>,
}

/// Request to delete multiple devices
#[derive(Debug, Serialize)]
pub struct DeleteDevicesRequest {
    /// List of device IDs to delete
    pub devices: Vec<String>,
    
    /// Authentication data for UIA
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthData>,
}

/// UIA flow information (for 401 responses)
#[derive(Debug, Deserialize)]
pub struct FlowInfo {
    /// Stages required to complete this authentication flow
    pub stages: Vec<String>,
}

/// UIA error response (401)
#[derive(Debug, Deserialize)]
pub struct UiaErrorResponse {
    /// List of stages already completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<Vec<String>>,
    
    /// Available authentication flows
    pub flows: Vec<FlowInfo>,
    
    /// Parameters for each authentication type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    
    /// Session ID to pass back to homeserver
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// Client for bulk device deletion
#[derive(Clone)]
pub struct DeleteDevicesClient {
    http_client: MatrixHttpClient,
}

impl DeleteDevicesClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Delete multiple devices
    /// 
    /// POST /_matrix/client/v3/delete_devices
    /// 
    /// This endpoint uses User-Interactive Authentication (UIA).
    /// Initial request may return 401 with UIA flows.
    /// 
    /// # Arguments
    /// * `request` - Device IDs to delete and optional auth data
    /// 
    /// # Returns
    /// * `Ok(())` on success (200)
    /// * `Err(HttpClientError::Matrix { status: 401, ... })` when UIA required
    ///   - Parse error body as `UiaErrorResponse` to get flows and session
    pub async fn delete_devices(
        &self,
        request: DeleteDevicesRequest,
    ) -> Result<(), HttpClientError> {
        // Delete returns empty JSON object {} on success
        let _: serde_json::Value = self
            .http_client
            .request(
                Method::POST,
                "/_matrix/client/v3/delete_devices",
                Some(&request),
            )
            .await?;
        
        Ok(())
    }
}
