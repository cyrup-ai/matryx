use axum::{
    Json,
    extract::{Path, State},
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
pub struct RoomKeyBackupRequest {
    pub sessions: std::collections::HashMap<String, RoomKeyBackupSession>,
}

#[derive(Serialize, Deserialize)]
pub struct RoomKeyBackupSession {
    pub first_message_index: u64,
    pub forwarded_count: u64,
    pub is_verified: bool,
    pub session_data: Value,
}

/// PUT /_matrix/client/v3/room_keys/keys/{roomId}
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<RoomKeyBackupRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Room key backup failed - authentication extraction failed: {}", e);
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

    // Get the current backup version for this user
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    let backup_version = backup_repo
        .get_latest_backup_version_string(&user_id)
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::PRECONDITION_FAILED)?;

    let mut count = 0;
    let mut etag = format!("{}", Utc::now().timestamp());

    // Store each session key
    for (session_id, session_data) in request.sessions {
        let encrypted_data = json!({
            "first_message_index": session_data.first_message_index,
            "forwarded_count": session_data.forwarded_count,
            "is_verified": session_data.is_verified,
            "session_data": session_data.session_data
        });

        backup_repo
            .store_room_key_raw(&user_id, &backup_version, &room_id, &session_id, &encrypted_data)
            .await
            .map_err(|e| {
                error!("Failed to store room key backup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        count += 1;
    }

    info!("Room keys backed up: user={} room={} sessions={}", user_id, room_id, count);

    Ok(Json(json!({
        "count": count,
        "etag": etag
    })))
}

/// GET /_matrix/client/v3/room_keys/keys/{roomId}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Room key backup get failed - authentication extraction failed: {}", e);
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

    // Get the current backup version
    let backup_repo = KeyBackupRepository::new(state.db.clone());
    let backup_version = backup_repo
        .get_latest_backup_version_string(&user_id)
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get all backed up sessions for this room
    let backups = backup_repo
        .get_room_keys_raw(&user_id, &backup_version, &room_id)
        .await
        .map_err(|e| {
            error!("Failed to get room key backups: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut sessions = std::collections::HashMap::new();
    
    for backup in backups {
        if let Some(session_id) = backup.get("session_id").and_then(|v| v.as_str()) {
            sessions.insert(session_id.to_string(), json!({
                "first_message_index": backup.get("first_message_index").unwrap_or(&json!(0)),
                "forwarded_count": backup.get("forwarded_count").unwrap_or(&json!(0)),
                "is_verified": backup.get("is_verified").unwrap_or(&json!(false)),
                "session_data": backup.get("session_data").unwrap_or(&json!({}))
            }));
        }
    }

    Ok(Json(json!({
        "sessions": sessions
    })))
}

/// DELETE /_matrix/client/v3/room_keys/keys/{roomId}
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Room key backup delete failed - authentication extraction failed: {}", e);
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

    // Delete all room key backups for this room
    backup_repo
        .delete_room_keys_for_room(&user_id, &backup_version, &room_id)
        .await
        .map_err(|e| {
            error!("Failed to delete room key backups: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Room key backups deleted: user={} room={}", user_id, room_id);

    Ok(Json(json!({
        "count": 0,
        "etag": format!("{}", Utc::now().timestamp())
    })))
}