use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};
use chrono::Utc;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::key_backup::KeyBackupRepository;

#[derive(Deserialize)]
pub struct BackupVersionCreateRequest {
    pub algorithm: String,
    pub auth_data: Value,
}

#[derive(Serialize, Deserialize)]
pub struct BackupVersionResponse {
    pub version: String,
}

/// POST /_matrix/client/v3/room_keys/version
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BackupVersionCreateRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Room key backup version create failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

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
    if !request.algorithm.starts_with("m.megolm_backup.v1.") {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Generate new version ID (timestamp-based)
    let version = format!("{}", Utc::now().timestamp());
    let etag = version.clone();

    // Create backup version
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    let version = backup_repo
        .create_backup_version(&user_id, &request.algorithm, &request.auth_data)
        .await
        .map_err(|e| {
            error!("Failed to create backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Backup version created: user={} version={}", user_id, version);

    Ok(Json(json!({
        "version": version
    })))
}

/// GET /_matrix/client/v3/room_keys/version
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Room key backup version get failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    // Get the latest backup version for this user
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    match backup_repo.get_latest_backup_version(&user_id).await {
        Ok(Some(backup)) => {
            Ok(Json(json!({
                "algorithm": backup.algorithm,
                "auth_data": backup.auth_data,
                "count": backup.count,
                "etag": backup.etag,
                "version": backup.version
            })))
        },
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to query backup versions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}