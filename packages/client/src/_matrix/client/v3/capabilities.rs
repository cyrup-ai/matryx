//! Matrix Client-Server API: Capabilities Negotiation
//!
//! Implementation of server capabilities discovery per Matrix spec v1.8
//! Reference: https://spec.matrix.org/v1.8/client-server-api/#capabilities-negotiation

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::Deserialize;
use std::collections::HashMap;

/// Boolean capability (enabled/disabled)
#[derive(Debug, Clone, Deserialize)]
pub struct BooleanCapability {
    /// True if the user can perform the action, false otherwise
    pub enabled: bool,
}

/// Room versions capability
#[derive(Debug, Clone, Deserialize)]
pub struct RoomVersionsCapability {
    /// Default room version for new rooms
    pub default: String,
    
    /// Map of room version to stability ("stable", "unstable")
    pub available: HashMap<String, String>,
}

/// Server capabilities response
#[derive(Debug, Clone, Deserialize)]
pub struct Capabilities {
    /// Capability to change password
    #[serde(rename = "m.change_password")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_password: Option<BooleanCapability>,
    
    /// Room versions supported
    #[serde(rename = "m.room_versions")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_versions: Option<RoomVersionsCapability>,
    
    /// Capability to change display name
    #[serde(rename = "m.set_displayname")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_displayname: Option<BooleanCapability>,
    
    /// Capability to change avatar URL
    #[serde(rename = "m.set_avatar_url")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_avatar_url: Option<BooleanCapability>,
    
    /// Capability to change 3PID associations
    #[serde(rename = "m.3pid_changes")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threepid_changes: Option<BooleanCapability>,
    
    /// Custom capabilities (server-specific)
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// Response from GET /_matrix/client/v3/capabilities
#[derive(Debug, Deserialize)]
pub struct CapabilitiesResponse {
    /// Server capabilities
    pub capabilities: Capabilities,
}

/// Client for server capabilities discovery
#[derive(Clone)]
pub struct CapabilitiesClient {
    http_client: MatrixHttpClient,
}

impl CapabilitiesClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get server capabilities
    /// 
    /// GET /_matrix/client/v3/capabilities
    /// 
    /// # Returns
    /// Server's supported feature set and capabilities
    pub async fn get_capabilities(&self) -> Result<CapabilitiesResponse, HttpClientError> {
        self.http_client
            .request(
                Method::GET,
                "/_matrix/client/v3/capabilities",
                None::<&()>,
            )
            .await
    }
}
