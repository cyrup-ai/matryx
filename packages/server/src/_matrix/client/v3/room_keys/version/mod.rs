use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info};
use matryx_surrealdb::KeyBackupRepository;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    crypto::MatryxCryptoProvider,
};


#[derive(Deserialize)]
pub struct CreateBackupVersionRequest {
    pub algorithm: String,
    pub auth_data: AuthData,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AuthData {
    pub public_key: String,
    pub signatures: HashMap<String, HashMap<String, String>>,
}

#[derive(Serialize)]
pub struct CreateBackupVersionResponse {
    pub version: String,
}

#[derive(Serialize)]
pub struct BackupVersionInfo {
    pub algorithm: String,
    pub auth_data: AuthData,
    pub count: u64,
    pub etag: String,
    pub version: String,
}



#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Invalid backup version")]
    InvalidVersion,
}

// Import the complete BackupVersion struct from by_version module
pub use by_version::BackupVersion;

/// GET /_matrix/client/v3/room_keys/version
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<BackupVersionInfo>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    // Get current backup version for user
    match get_current_backup_version(&state.db, &user_id).await {
        Ok(Some(backup_version)) => {
            // Convert auth_data from serde_json::Value to AuthData struct
            let auth_data: AuthData = serde_json::from_value(backup_version.auth_data.clone())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let response = BackupVersionInfo {
                algorithm: backup_version.algorithm.clone(),
                auth_data,
                count: backup_version.count,
                etag: backup_version.etag.clone(),
                version: backup_version.version.clone(),
            };
            
            info!("Retrieved backup version {} (created: {})", backup_version.version, backup_version.created_at);
            Ok(Json(response))
        },
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// POST /_matrix/client/v3/room_keys/version
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateBackupVersionRequest>,
) -> Result<Json<CreateBackupVersionResponse>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    // Validate algorithm
    if request.algorithm != "m.megolm_backup.v1.curve25519-aes-sha2" {
        error!("Invalid backup algorithm: {}", request.algorithm);
        return Err(StatusCode::BAD_REQUEST);
    }

    let crypto_provider = MatryxCryptoProvider::new(state.db.clone());

    // NEW: Validate backup auth data using vodozemac
    let crypto_auth_data = crate::crypto::AuthData {
        public_key: request.auth_data.public_key.clone(),
        signatures: request.auth_data.signatures.clone(),
    };
    if !crypto_provider
        .validate_backup_auth_data(&crypto_auth_data, &user_id)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        error!("Invalid backup auth data for user: {}", user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create new backup version
    let version = create_backup_version(&state.db, &user_id, &request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!("Created room key backup version {} for user: {}", version, user_id);

    Ok(Json(CreateBackupVersionResponse { version }))
}



// Database operations
pub async fn get_current_backup_version(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
) -> Result<Option<BackupVersion>, BackupError> {
    let backup_repo = KeyBackupRepository::new(db.clone());
    match backup_repo.get_latest_backup_version(user_id).await {
        Ok(Some(backup_version)) => {
            let count = get_backup_count(db, user_id, &backup_version.version)
                .await
                .unwrap_or(0);
            let etag = generate_backup_etag(user_id, &backup_version.version);
            
            Ok(Some(BackupVersion {
                version: backup_version.version.clone(),
                algorithm: backup_version.algorithm,
                auth_data: backup_version.auth_data,
                count,
                etag,
                user_id: user_id.to_string(),
                created_at: chrono::Utc::now(), // Repository doesn't provide timestamp
                updated_at: chrono::Utc::now(), // Repository doesn't provide timestamp
            }))
        },
        Ok(None) => Ok(None),
        Err(e) => Err(BackupError::Database(e.to_string())),
    }
}

async fn create_backup_version(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    request: &CreateBackupVersionRequest,
) -> Result<String, BackupError> {
    let backup_repo = KeyBackupRepository::new(db.clone());
    let auth_data_json = serde_json::to_value(&request.auth_data)
        .map_err(|e| BackupError::Crypto(format!("Failed to serialize auth_data: {}", e)))?;
    
    backup_repo
        .create_backup_version(user_id, &request.algorithm, &auth_data_json)
        .await
        .map_err(|e| BackupError::Database(e.to_string()))
}

pub async fn store_room_key(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    version: &str,
    room_id: &str,
    session_id: &str,
    encrypted_data: &Value,
) -> Result<(), BackupError> {
    let backup_repo = KeyBackupRepository::new(db.clone());
    backup_repo
        .store_room_key_raw(user_id, version, room_id, session_id, encrypted_data)
        .await
        .map_err(|e| BackupError::Database(e.to_string()))
}

pub async fn get_backup_count(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    version: &str,
) -> Result<u64, BackupError> {
    let backup_repo = KeyBackupRepository::new(db.clone());
    backup_repo
        .get_backup_count(user_id, version)
        .await
        .map_err(|e| BackupError::Database(e.to_string()))
}

pub fn generate_backup_etag(user_id: &str, version: &str) -> String {
    format!("{}:{}", user_id, version)
}

// Helper function to extract version from query parameters
pub fn extract_version_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    // In a real implementation, this would parse query parameters
    // For now, we'll look for a custom header (this is a simplified approach)
    headers
        .get("X-Matrix-Backup-Version")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

// Validate backup version format (Matrix spec requires numeric versions)
pub fn is_valid_backup_version(version: &str) -> bool {
    // Version must be a positive integer string
    version.parse::<u64>().is_ok() && !version.starts_with('0') && !version.is_empty()
}

// Validate backup version and return appropriate error
pub async fn validate_backup_version(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    version: &str,
) -> Result<BackupVersion, BackupError> {
    // Check format first
    if !is_valid_backup_version(version) {
        return Err(BackupError::InvalidVersion);
    }
    
    // Check if version exists for user
    let backup_repo = KeyBackupRepository::new(db.clone());
    match backup_repo.get_backup_version(user_id, version).await {
        Ok(Some(backup_version)) => {
            let count = get_backup_count(db, user_id, version)
                .await
                .unwrap_or(0);
            let etag = generate_backup_etag(user_id, version);
            
            Ok(BackupVersion {
                version: backup_version.version,
                algorithm: backup_version.algorithm,
                auth_data: backup_version.auth_data,
                count,
                etag,
                user_id: user_id.to_string(),
                created_at: chrono::Utc::now(), // Repository doesn't provide timestamp
                updated_at: chrono::Utc::now(), // Repository doesn't provide timestamp
            })
        },
        Ok(None) => Err(BackupError::InvalidVersion),
        Err(e) => Err(BackupError::Database(e.to_string())),
    }
}

pub mod by_version;
