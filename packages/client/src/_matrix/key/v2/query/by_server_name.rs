//! Matrix Server-Server API: Server Key Query by Server Name
//!
//! Implementation of notary server key query endpoint
//! Reference: https://spec.matrix.org/v1.8/server-server-api/#get_matrixkeyv2queryservername

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server key response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeyResponse {
    /// Array of server key objects
    pub server_keys: Vec<ServerKeyInfo>,
}

/// Individual server key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeyInfo {
    /// The homeserver's server name
    pub server_name: String,
    
    /// Public keys of the homeserver for verifying digital signatures
    /// Map of algorithm:version to key object
    pub verify_keys: HashMap<String, VerifyKey>,
    
    /// The public keys that the server used to use and when it stopped using them
    /// Map of algorithm:version to old key object
    #[serde(default)]
    pub old_verify_keys: HashMap<String, OldVerifyKey>,
    
    /// POSIX timestamp when the list of valid keys should be refreshed
    /// Keys used beyond this timestamp MUST be considered invalid
    pub valid_until_ts: i64,
    
    /// Digital signatures for this object signed using the verify_keys
    /// Outer map: server name, Inner map: algorithm:version to signature
    pub signatures: HashMap<String, HashMap<String, String>>,
}

/// Verify key object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKey {
    /// The unpadded base64-encoded key
    pub key: String,
}

/// Old verify key object with expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OldVerifyKey {
    /// The unpadded base64-encoded key
    pub key: String,
    
    /// POSIX timestamp in milliseconds for when this key expired
    pub expired_ts: i64,
}

/// Client for server key query operations
#[derive(Clone)]
pub struct ServerKeyQueryClient {
    http_client: MatrixHttpClient,
}

impl ServerKeyQueryClient {
    /// Create a new server key query client
    ///
    /// # Arguments
    /// * `http_client` - HTTP client configured with homeserver URL
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Query for another server's keys through a notary server
    ///
    /// Endpoint: GET /_matrix/key/v2/query/{serverName}
    /// Authentication: Not required (public endpoint)
    ///
    /// # Arguments
    /// * `server_name` - The server name to query
    /// * `minimum_valid_until_ts` - Optional minimum validity timestamp
    ///
    /// # Returns
    /// * `Result<ServerKeyResponse, HttpClientError>` - Server keys or error
    ///
    /// # Matrix Spec
    /// Query for another server's keys through a notary server.
    /// The receiving (notary) server must sign the keys returned by the queried server.
    /// Returns empty array if server could not be reached and no cached keys available.
    pub async fn query_server_keys(
        &self,
        server_name: &str,
        minimum_valid_until_ts: Option<i64>,
    ) -> Result<ServerKeyResponse, HttpClientError> {
        let mut path = format!("/_matrix/key/v2/query/{}", server_name);
        
        if let Some(min_valid) = minimum_valid_until_ts {
            path.push_str(&format!("?minimum_valid_until_ts={}", min_valid));
        }

        self.http_client
            .request(Method::GET, &path, None::<&()>)
            .await
    }
}
