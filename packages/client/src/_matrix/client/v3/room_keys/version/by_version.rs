//! Matrix Client-Server API: Room Keys Backup Version Management
//!
//! Implementation of backup version endpoints per Matrix spec v1.8
//! Reference: https://spec.matrix.org/v1.8/client-server-api/#server-side-key-backups

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// Backup algorithm: m.megolm_backup.v1.curve25519-aes-sha2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupAuthData {
    /// Curve25519 public key used to encrypt backups (unpadded base64)
    pub public_key: String,
    
    /// Signatures of the auth_data (Signed JSON format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<serde_json::Value>,
}

/// Request to create a new backup version
#[derive(Debug, Serialize)]
pub struct CreateBackupRequest {
    /// Algorithm name (e.g., "m.megolm_backup.v1.curve25519-aes-sha2")
    pub algorithm: String,
    
    /// Algorithm-specific authentication data
    pub auth_data: BackupAuthData,
}

/// Response from creating backup version
#[derive(Debug, Deserialize)]
pub struct CreateBackupResponse {
    /// The backup version string
    pub version: String,
}

/// Backup version information
#[derive(Debug, Clone, Deserialize)]
pub struct BackupInfo {
    /// Algorithm name
    pub algorithm: String,
    
    /// Algorithm-specific auth data
    pub auth_data: BackupAuthData,
    
    /// Count of keys in this backup
    pub count: u32,
    
    /// Etag representing current backup state
    pub etag: String,
    
    /// Version identifier
    pub version: String,
}

/// Client for backup version management
#[derive(Clone)]
pub struct BackupVersionClient {
    http_client: MatrixHttpClient,
}

impl BackupVersionClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Create a new backup version
    /// 
    /// POST /_matrix/client/v3/room_keys/version
    /// 
    /// # Arguments
    /// * `request` - Algorithm and auth data for new backup
    /// 
    /// # Returns
    /// The new backup version string
    pub async fn create_version(
        &self,
        request: CreateBackupRequest,
    ) -> Result<CreateBackupResponse, HttpClientError> {
        self.http_client
            .request(
                Method::POST,
                "/_matrix/client/v3/room_keys/version",
                Some(&request),
            )
            .await
    }

    /// Get backup version information
    /// 
    /// GET /_matrix/client/v3/room_keys/version/{version}
    /// 
    /// # Arguments
    /// * `version` - The backup version to query
    pub async fn get_version(
        &self,
        version: &str,
    ) -> Result<BackupInfo, HttpClientError> {
        let path = format!("/_matrix/client/v3/room_keys/version/{}", version);
        
        self.http_client
            .request(Method::GET, &path, None::<&()>)
            .await
    }

    /// Get the latest backup version information
    /// 
    /// GET /_matrix/client/v3/room_keys/version
    pub async fn get_latest_version(&self) -> Result<BackupInfo, HttpClientError> {
        self.http_client
            .request(
                Method::GET,
                "/_matrix/client/v3/room_keys/version",
                None::<&()>,
            )
            .await
    }

    /// Delete a backup version
    /// 
    /// DELETE /_matrix/client/v3/room_keys/version/{version}
    pub async fn delete_version(
        &self,
        version: &str,
    ) -> Result<(), HttpClientError> {
        let path = format!("/_matrix/client/v3/room_keys/version/{}", version);
        
        // Delete returns empty JSON object {}
        let _: serde_json::Value = self
            .http_client
            .request(Method::DELETE, &path, None::<&()>)
            .await?;
        
        Ok(())
    }
}
