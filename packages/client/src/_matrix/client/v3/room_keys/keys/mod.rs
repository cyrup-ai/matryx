//! Matrix Client-Server API: Room Keys Backup
//!
//! Implementation of room key backup endpoints per Matrix spec v1.8
//! Reference: https://spec.matrix.org/v1.8/client-server-api/#server-side-key-backups

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Session key backup data per Matrix spec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBackupData {
    /// Index of the first message in the session that the key can decrypt
    pub first_message_index: u32,
    
    /// Number of times this key has been forwarded via key-sharing
    pub forwarded_count: u32,
    
    /// Whether the device backing up verified the device the key is from
    pub is_verified: bool,
    
    /// Algorithm-dependent encrypted backup data
    /// For m.megolm_backup.v1.curve25519-aes-sha2:
    /// - ciphertext: base64 encrypted session key
    /// - ephemeral: base64 ephemeral public key
    /// - mac: base64 MAC of ciphertext
    pub session_data: serde_json::Value,
}

/// Room key backup structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeyBackup {
    /// Map of session IDs to key backup data
    pub sessions: HashMap<String, KeyBackupData>,
}

/// Request body for PUT /_matrix/client/v3/room_keys/keys
#[derive(Debug, Serialize)]
pub struct BackupKeysRequest {
    /// Map of room IDs to room key backups
    pub rooms: HashMap<String, RoomKeyBackup>,
}

/// Response from PUT /_matrix/client/v3/room_keys/keys
#[derive(Debug, Deserialize)]
pub struct BackupKeysResponse {
    /// Number of keys stored in the backup
    pub count: u32,
    
    /// New etag value representing stored keys in backup
    pub etag: String,
}

/// Response from GET /_matrix/client/v3/room_keys/keys
#[derive(Debug, Deserialize)]
pub struct GetKeysResponse {
    /// Map of room IDs to room key backups
    pub rooms: HashMap<String, RoomKeyBackup>,
}

/// Client for room key backup operations
#[derive(Clone)]
pub struct RoomKeysClient {
    http_client: MatrixHttpClient,
}

impl RoomKeysClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Backup room keys to server
    /// 
    /// PUT /_matrix/client/v3/room_keys/keys?version={version}
    /// 
    /// # Arguments
    /// * `version` - The backup version to store keys in (must be current)
    /// * `request` - Keys to backup
    /// 
    /// # Returns
    /// Count and etag of stored keys
    pub async fn backup_keys(
        &self,
        version: &str,
        request: BackupKeysRequest,
    ) -> Result<BackupKeysResponse, HttpClientError> {
        let path = format!("/_matrix/client/v3/room_keys/keys?version={}", version);
        
        self.http_client
            .request(Method::PUT, &path, Some(&request))
            .await
    }

    /// Retrieve room keys from backup
    /// 
    /// GET /_matrix/client/v3/room_keys/keys?version={version}
    /// 
    /// # Arguments
    /// * `version` - The backup version to retrieve from
    /// 
    /// # Returns
    /// All backed up room keys
    pub async fn get_keys(
        &self,
        version: &str,
    ) -> Result<GetKeysResponse, HttpClientError> {
        let path = format!("/_matrix/client/v3/room_keys/keys?version={}", version);
        
        self.http_client
            .request(Method::GET, &path, None::<&()>)
            .await
    }

    /// Delete all room keys from backup
    /// 
    /// DELETE /_matrix/client/v3/room_keys/keys?version={version}
    pub async fn delete_keys(
        &self,
        version: &str,
    ) -> Result<BackupKeysResponse, HttpClientError> {
        let path = format!("/_matrix/client/v3/room_keys/keys?version={}", version);
        
        self.http_client
            .request(Method::DELETE, &path, None::<&()>)
            .await
    }
}
