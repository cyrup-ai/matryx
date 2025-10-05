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
use matryx_surrealdb::repository::key_backup::KeyBackupRepository;

#[derive(Serialize, Deserialize)]
pub struct SessionKeyBackup {
    pub first_message_index: u64,
    pub forwarded_count: u64,
    pub is_verified: bool,
    pub session_data: Value,
}

/// DELETE /_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((room_id, session_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Session key backup delete failed - authentication extraction failed: {}", e);
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

    // Get current backup version
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    let backup_version = backup_repo
        .get_latest_backup_version_string(&user_id)
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Delete specific session key backup
    backup_repo
        .delete_room_key(&user_id, &backup_version, &room_id, &session_id)
        .await
        .map_err(|e| {
            error!("Failed to delete session key backup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Session key backup deleted: user={} room={} session={}", user_id, room_id, session_id);

    Ok(Json(json!({
        "count": 0,
        "etag": format!("{}", Utc::now().timestamp())
    })))
}

/// GET /_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((room_id, session_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Session key backup get failed - authentication extraction failed: {}", e);
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

    // Get current backup version
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    let backup_version = backup_repo
        .get_latest_backup_version_string(&user_id)
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get specific session key backup
    match backup_repo
        .get_room_key(&user_id, &backup_version, &room_id, &session_id)
        .await
    {
        Ok(Some(room_key)) => Ok(Json(json!({
            "first_message_index": room_key.first_message_index,
            "forwarded_count": room_key.forwarded_count,
            "is_verified": room_key.is_verified,
            "session_data": room_key.session_data
        }))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get session key backup: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

/// PUT /_matrix/client/v3/room_keys/keys/{roomId}/{sessionId}
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((room_id, session_id)): Path<(String, String)>,
    Json(request): Json<SessionKeyBackup>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Session key backup put failed - authentication extraction failed: {}", e);
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

    // Get current backup version
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    let backup_version = backup_repo
        .get_latest_backup_version_string(&user_id)
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::PRECONDITION_FAILED)?;

    // Store session key backup
    let encrypted_data = json!({
        "first_message_index": request.first_message_index,
        "forwarded_count": request.forwarded_count,
        "is_verified": request.is_verified,
        "session_data": request.session_data
    });

    backup_repo
        .store_room_key_raw(&user_id, &backup_version, &room_id, &session_id, &encrypted_data)
        .await
        .map_err(|e| {
            error!("Failed to store session key backup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let etag = format!("{}", Utc::now().timestamp());

    info!("Session key backup stored: user={} room={} session={}", user_id, room_id, session_id);

    Ok(Json(json!({
        "count": 1,
        "etag": etag
    })))
}
