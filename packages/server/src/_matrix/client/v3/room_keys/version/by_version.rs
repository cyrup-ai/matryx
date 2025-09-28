use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};
use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::KeyBackupRepository;


#[derive(Deserialize)]
pub struct BackupVersionUpdateRequest {
    pub algorithm: String,
    pub auth_data: Value,
}

#[derive(Serialize, Deserialize)]
pub struct BackupVersion {
    pub version: String,
    pub algorithm: String,
    pub auth_data: Value,
    pub count: u64,
    pub etag: String,
    pub user_id: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

/// DELETE /_matrix/client/v3/room_keys/version/{version}
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Room key backup version delete failed - authentication extraction failed: {}", e);
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

    // Delete backup version and all associated keys
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    backup_repo
        .delete_backup_version(&user_id, &version)
        .await
        .map_err(|e| {
            error!("Failed to delete backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Backup version deleted: user={} version={}", user_id, version);
    Ok(Json(json!({})))
}

/// GET /_matrix/client/v3/room_keys/version/{version}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
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

    // Get backup version info
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    match backup_repo.get_backup_version(&user_id, &version).await {
        Ok(Some(backup)) => {
            let count = backup_repo
                .get_backup_count(&user_id, &version)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(Json(json!({
                "algorithm": backup.algorithm,
                "auth_data": backup.auth_data,
                "count": count,
                "etag": backup.etag,
                "version": backup.version
            })))
        },
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to query backup version: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// PUT /_matrix/client/v3/room_keys/version/{version}
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version): Path<String>,
    Json(request): Json<BackupVersionUpdateRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Room key backup version update failed - authentication extraction failed: {}", e);
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

    // Update backup version
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    backup_repo
        .update_backup_version(&user_id, &version, &request.auth_data)
        .await
        .map_err(|e| {
            error!("Failed to update backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Backup version updated: user={} version={}", user_id, version);
    Ok(Json(json!({})))
}
