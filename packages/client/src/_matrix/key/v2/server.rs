//! Matrix Server-Server API: Get Server Keys
//!
//! Implementation of server key retrieval endpoint
//! Reference: https://spec.matrix.org/v1.8/server-server-api/#get_matrixkeyv2server

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server keys information structure
///
/// Contains the homeserver's published signing keys per Matrix specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeysInfo {
    /// The homeserver's server name
    pub server_name: String,
    
    /// Public keys of the homeserver for verifying digital signatures
    /// The object key is the algorithm and version combined (e.g., ed25519:abc123)
    /// Version must have characters matching [a-zA-Z0-9_]
    pub verify_keys: HashMap<String, VerifyKey>,
    
    /// The public keys that the server used to use and when it stopped using them
    /// Same key format as verify_keys
    #[serde(default)]
    pub old_verify_keys: HashMap<String, OldVerifyKey>,
    
    /// POSIX timestamp in milliseconds when the list of valid keys should be refreshed
    /// Keys used beyond this timestamp MUST be considered invalid
    /// Servers MUST use the lesser of this field and 7 days into the future
    pub valid_until_ts: i64,
    
    /// Digital signatures for this object signed using the verify_keys
    /// Outer map: server name, Inner map: algorithm:key_id to signature
    pub signatures: HashMap<String, HashMap<String, String>>,
}

/// Verify key object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKey {
    /// The unpadded base64-encoded key
    pub key: String,
}

/// Old verify key object with expiration timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OldVerifyKey {
    /// The unpadded base64-encoded key
    pub key: String,
    
    /// POSIX timestamp in milliseconds for when this key expired
    pub expired_ts: i64,
}

/// Client for server key operations
#[derive(Clone)]
pub struct ServerKeyClient {
    http_client: MatrixHttpClient,
}

impl ServerKeyClient {
    /// Create a new server key client
    ///
    /// # Arguments
    /// * `http_client` - HTTP client configured with homeserver URL
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get the homeserver's published signing keys
    ///
    /// Endpoint: GET /_matrix/key/v2/server
    /// Authentication: Not required (public endpoint)
    ///
    /// # Returns
    /// * `Result<ServerKeysInfo, HttpClientError>` - Server keys or error
    ///
    /// # Matrix Spec
    /// Gets the homeserver's published signing keys. The homeserver may have
    /// any number of active keys and may have a number of old keys.
    ///
    /// Intermediate notary servers should cache a response for half of its lifetime.
    /// Originating servers should avoid returning responses that expire in less
    /// than an hour to avoid repeated requests.
    pub async fn get_server_keys(&self) -> Result<ServerKeysInfo, HttpClientError> {
        self.http_client
            .request(Method::GET, "/_matrix/key/v2/server", None::<&()>)
            .await
    }
}
