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
    let auth = extract_matrix_auth(&headers).map_err(|e| {
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
    let delete_keys_query =
        "DELETE FROM room_key_backups WHERE user_id = $user_id AND version = $version";
    let _keys_result = state
        .db
        .query(delete_keys_query)
        .bind(("user_id", user_id.clone()))
        .bind(("version", version.clone()))
        .await
        .map_err(|e| {
            error!("Failed to delete room key backups: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let delete_version_query =
        "DELETE FROM backup_versions WHERE user_id = $user_id AND version = $version";
    let mut version_result = state
        .db
        .query(delete_version_query)
        .bind(("user_id", user_id.clone()))
        .bind(("version", version.clone()))
        .await
        .map_err(|e| {
            error!("Failed to delete backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let deleted: Option<Value> =
        version_result.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if deleted.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    info!("Backup version deleted: user={} version={}", user_id, version);
    Ok(Json(json!({})))
}

/// GET /_matrix/client/v3/room_keys/version/{version}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version): Path<String>,
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

    // Get backup version info
    let query = "SELECT * FROM backup_versions WHERE user_id = $user_id AND version = $version";
    let mut response = state
        .db
        .query(query)
        .bind(("user_id", user_id.clone()))
        .bind(("version", version.clone()))
        .await
        .map_err(|e| {
            error!("Failed to query backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let backup_version: Option<BackupVersion> =
        response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(backup) = backup_version {
        // Count backed up keys for this version
        let count_query = "SELECT count() FROM room_key_backups WHERE user_id = $user_id AND version = $version GROUP ALL";
        let mut count_response = state
            .db
            .query(count_query)
            .bind(("user_id", user_id))
            .bind(("version", version))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let count: u64 = count_response
            .take::<Option<u64>>(0)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .unwrap_or(0);

        Ok(Json(json!({
            "algorithm": backup.algorithm,
            "auth_data": backup.auth_data,
            "count": count,
            "etag": backup.etag,
            "version": backup.version
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// PUT /_matrix/client/v3/room_keys/version/{version}
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version): Path<String>,
    Json(request): Json<BackupVersionUpdateRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
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
    let new_etag = format!("{}", Utc::now().timestamp());
    let query = "
        UPDATE backup_versions 
        SET algorithm = $algorithm, auth_data = $auth_data, etag = $etag, updated_at = $updated_at
        WHERE user_id = $user_id AND version = $version
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("user_id", user_id.clone()))
        .bind(("version", version.clone()))
        .bind(("algorithm", request.algorithm.clone()))
        .bind(("auth_data", request.auth_data.clone()))
        .bind(("etag", new_etag.clone()))
        .bind(("updated_at", Utc::now()))
        .await
        .map_err(|e| {
            error!("Failed to update backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let updated: Option<Value> = response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if updated.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    info!("Backup version updated: user={} version={}", user_id, version);
    Ok(Json(json!({})))
}
