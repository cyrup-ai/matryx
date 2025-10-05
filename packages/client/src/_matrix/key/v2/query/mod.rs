//! Matrix Client-Server API: Key Query
//!
//! Implementation of key query endpoints for end-to-end encryption
//! Reference: https://spec.matrix.org/v1.8/client-server-api/#post_matrixclientv3keysquery

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub mod by_server_name;

/// Request for querying device keys
#[derive(Debug, Serialize)]
pub struct KeyQueryRequest {
    /// The keys to be downloaded
    /// Maps user IDs to a list of device IDs, or empty list for all devices
    pub device_keys: HashMap<String, Vec<String>>,
    
    /// If included, results include the current one-time key counts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    
    /// Token from previous sync (for tracking device list changes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Response containing device keys and master keys
#[derive(Debug, Deserialize)]
pub struct KeyQueryResponse {
    /// Information on device keys for users
    /// Maps user ID to map of device ID to device keys
    #[serde(default)]
    pub device_keys: HashMap<String, HashMap<String, DeviceKeys>>,
    
    /// Information on master keys for users
    /// Maps user ID to master key info
    #[serde(default)]
    pub master_keys: HashMap<String, CrossSigningKey>,
    
    /// Information on self-signing keys for users
    /// Maps user ID to self-signing key info
    #[serde(default)]
    pub self_signing_keys: HashMap<String, CrossSigningKey>,
    
    /// Information on user-signing keys for users
    /// Maps user ID to user-signing key info
    #[serde(default)]
    pub user_signing_keys: HashMap<String, CrossSigningKey>,
    
    /// Information on failures for any user/device queries
    #[serde(default)]
    pub failures: HashMap<String, Value>,
}

/// Device key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKeys {
    /// User ID that owns this device
    pub user_id: String,
    
    /// Device ID
    pub device_id: String,
    
    /// Encryption algorithms supported by this device
    pub algorithms: Vec<String>,
    
    /// Public identity keys
    /// Map of algorithm:key_id to base64-encoded key
    pub keys: HashMap<String, String>,
    
    /// Signatures for this device key object
    /// Map of user_id to map of algorithm:key_id to signature
    pub signatures: HashMap<String, HashMap<String, String>>,
    
    /// Additional user-specified data
    #[serde(default)]
    pub unsigned: Option<Value>,
}

/// Cross-signing key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningKey {
    /// User ID that owns this key
    pub user_id: String,
    
    /// What the key is used for
    pub usage: Vec<String>,
    
    /// Public identity keys
    /// Map of algorithm:key_id to base64-encoded key
    pub keys: HashMap<String, String>,
    
    /// Signatures for this key object
    /// Map of user_id to map of algorithm:key_id to signature
    pub signatures: HashMap<String, HashMap<String, String>>,
}

/// Client for key query operations
#[derive(Clone)]
pub struct KeyClient {
    http_client: MatrixHttpClient,
}

impl KeyClient {
    /// Create a new key client
    ///
    /// # Arguments
    /// * `http_client` - HTTP client configured with homeserver URL
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Query device keys for users
    ///
    /// Endpoint: POST /_matrix/client/v3/keys/query
    /// Authentication: Required
    ///
    /// # Arguments
    /// * `request` - Key query request specifying users and devices
    ///
    /// # Returns
    /// * `Result<KeyQueryResponse, HttpClientError>` - Device and master keys or error
    ///
    /// # Matrix Spec
    /// Returns the current devices and identity keys for the given users.
    /// Essential for end-to-end encryption key distribution.
    pub async fn query_keys(
        &self,
        request: &KeyQueryRequest,
    ) -> Result<KeyQueryResponse, HttpClientError> {
        self.http_client
            .request(
                Method::POST,
                "/_matrix/client/v3/keys/query",
                Some(request),
            )
            .await
    }

    /// Convenience method: Query all devices for specific users
    ///
    /// # Arguments
    /// * `user_ids` - List of user IDs to query
    ///
    /// # Returns
    /// * `Result<KeyQueryResponse, HttpClientError>` - Device and master keys or error
    pub async fn query_users(
        &self,
        user_ids: Vec<String>,
    ) -> Result<KeyQueryResponse, HttpClientError> {
        let device_keys: HashMap<String, Vec<String>> = user_ids
            .into_iter()
            .map(|user_id| (user_id, vec![]))
            .collect();

        let request = KeyQueryRequest {
            device_keys,
            timeout: None,
            token: None,
        };

        self.query_keys(&request).await
    }
}
