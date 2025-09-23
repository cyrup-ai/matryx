use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{error, info};

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

#[derive(Deserialize)]
pub struct RoomKeyBackupData {
    pub first_message_index: u64,
    pub forwarded_count: u64,
    pub is_verified: bool,
    pub session_data: Value,
}

#[derive(Serialize)]
pub struct RoomKeyBackupResponse {
    pub etag: String,
    pub count: u64,
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

pub struct BackupVersion {
    pub version: String,
    pub algorithm: String,
    pub auth_data: AuthData,
    pub created_at: String,
}

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
            let count = get_backup_count(&state.db, &user_id, &backup_version.version)
                .await
                .unwrap_or(0);
            let etag = generate_backup_etag(&user_id, &backup_version.version);

            Ok(Json(BackupVersionInfo {
                algorithm: backup_version.algorithm,
                auth_data: backup_version.auth_data,
                count,
                etag,
                version: backup_version.version,
            }))
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

/// PUT /_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}
pub async fn put_room_key(
    State(state): State<AppState>,
    Path((room_id, session_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<RoomKeyBackupData>,
) -> Result<Json<RoomKeyBackupResponse>, StatusCode> {
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

    // Get current backup version
    let current_version = get_current_backup_version(&state.db, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // NEW: Validate and encrypt room key using vodozemac
    let crypto_provider = MatryxCryptoProvider::new(state.db.clone());
    let crypto_room_key_data =
        crate::crypto::RoomKeyBackupData { session_data: request.session_data.clone() };
    let crypto_auth_data = crate::crypto::AuthData {
        public_key: current_version.auth_data.public_key.clone(),
        signatures: current_version.auth_data.signatures.clone(),
    };
    let encrypted_key_data = crypto_provider
        .encrypt_room_key_for_backup(&crypto_room_key_data, &crypto_auth_data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Store encrypted room key backup
    let encrypted_value =
        serde_json::to_value(&encrypted_key_data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    store_room_key(
        &state.db,
        &user_id,
        &current_version.version,
        &room_id,
        &session_id,
        &encrypted_value,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let count = get_backup_count(&state.db, &user_id, &current_version.version)
        .await
        .unwrap_or(0);
    let etag = generate_backup_etag(&user_id, &current_version.version);

    info!("Stored room key backup for user: {} room: {} session: {}", user_id, room_id, session_id);

    Ok(Json(RoomKeyBackupResponse { etag, count }))
}

// Database operations
async fn get_current_backup_version(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
) -> Result<Option<BackupVersion>, BackupError> {
    let query = "SELECT * FROM room_key_backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let user_id_owned = user_id.to_string();
    let mut response = db
        .query(query)
        .bind(("user_id", user_id_owned))
        .await
        .map_err(|e| BackupError::Database(e.to_string()))?;

    let backup_versions: Vec<Value> =
        response.take(0).map_err(|e| BackupError::Database(e.to_string()))?;

    if let Some(version_data) = backup_versions.first() {
        let version = version_data
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or(BackupError::InvalidVersion)?
            .to_string();

        let algorithm = version_data
            .get("algorithm")
            .and_then(|v| v.as_str())
            .unwrap_or("m.megolm_backup.v1.curve25519-aes-sha2")
            .to_string();

        let auth_data: AuthData =
            serde_json::from_value(version_data.get("auth_data").cloned().unwrap_or(Value::Null))
                .map_err(|e| BackupError::Database(e.to_string()))?;

        let created_at = version_data
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(Some(BackupVersion { version, algorithm, auth_data, created_at }))
    } else {
        Ok(None)
    }
}

async fn create_backup_version(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    request: &CreateBackupVersionRequest,
) -> Result<String, BackupError> {
    let version = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    let query = r#"
        CREATE room_key_backup_versions CONTENT {
            user_id: $user_id,
            version: $version,
            algorithm: $algorithm,
            auth_data: $auth_data,
            created_at: $created_at
        }
    "#;

    let user_id_owned = user_id.to_string();
    let version_owned = version.clone();
    let algorithm_owned = request.algorithm.clone();
    let auth_data_owned = request.auth_data.clone();
    let created_at_owned = created_at.clone();

    db.query(query)
        .bind(("user_id", user_id_owned))
        .bind(("version", version_owned))
        .bind(("algorithm", algorithm_owned))
        .bind(("auth_data", auth_data_owned))
        .bind(("created_at", created_at_owned))
        .await
        .map_err(|e| BackupError::Database(e.to_string()))?;

    Ok(version)
}

async fn store_room_key(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    version: &str,
    room_id: &str,
    session_id: &str,
    encrypted_data: &Value,
) -> Result<(), BackupError> {
    let query = r#"
        CREATE room_key_backups CONTENT {
            user_id: $user_id,
            backup_version: $version,
            room_id: $room_id,
            session_id: $session_id,
            encrypted_data: $encrypted_data,
            created_at: time::now()
        }
    "#;

    let user_id_owned = user_id.to_string();
    let version_owned = version.to_string();
    let room_id_owned = room_id.to_string();
    let session_id_owned = session_id.to_string();
    let encrypted_data_owned = encrypted_data.clone();

    db.query(query)
        .bind(("user_id", user_id_owned))
        .bind(("version", version_owned))
        .bind(("room_id", room_id_owned))
        .bind(("session_id", session_id_owned))
        .bind(("encrypted_data", encrypted_data_owned))
        .await
        .map_err(|e| BackupError::Database(e.to_string()))?;

    Ok(())
}

async fn get_backup_count(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    version: &str,
) -> Result<u64, BackupError> {
    let query = "SELECT count() AS count FROM room_key_backups WHERE user_id = $user_id AND backup_version = $version GROUP ALL";
    let user_id_owned = user_id.to_string();
    let version_owned = version.to_string();
    let mut response = db
        .query(query)
        .bind(("user_id", user_id_owned))
        .bind(("version", version_owned))
        .await
        .map_err(|e| BackupError::Database(e.to_string()))?;

    let result: Option<Value> =
        response.take(0).map_err(|e| BackupError::Database(e.to_string()))?;

    if let Some(count_data) = result {
        if let Some(count) = count_data.get("count").and_then(|v| v.as_u64()) {
            Ok(count)
        } else {
            Ok(0)
        }
    } else {
        Ok(0)
    }
}

fn generate_backup_etag(user_id: &str, version: &str) -> String {
    format!("{}:{}", user_id, version)
}

pub mod by_version;
