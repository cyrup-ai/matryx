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
    let _: Option<Value> = state.db
        .create(("backup_versions", format!("{}:{}", user_id, version)))
        .content(json!({
            "user_id": user_id,
            "version": version,
            "algorithm": request.algorithm,
            "auth_data": request.auth_data,
            "count": 0,
            "etag": etag,
            "created_at": Utc::now(),
            "updated_at": Utc::now()
        }))
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
    let query = "SELECT * FROM backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let mut response = state.db
        .query(query)
        .bind(("user_id", user_id.clone()))
        .await
        .map_err(|e| {
            error!("Failed to query backup versions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let backup_version: Option<Value> = response.take(0)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(backup) = backup_version {
        Ok(Json(json!({
            "algorithm": backup.get("algorithm").unwrap_or(&json!("")),
            "auth_data": backup.get("auth_data").unwrap_or(&json!({})),
            "count": backup.get("count").unwrap_or(&json!(0)),
            "etag": backup.get("etag").unwrap_or(&json!("")),
            "version": backup.get("version").unwrap_or(&json!(""))
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}