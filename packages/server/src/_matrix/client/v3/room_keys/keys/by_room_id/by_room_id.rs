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
    let version_query = "SELECT version FROM backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let mut version_response = state.db
        .query(version_query)
        .bind(("user_id", &user_id))
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let version: Option<String> = version_response.take(0)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let backup_version = version.ok_or(StatusCode::PRECONDITION_FAILED)?;

    let mut count = 0;
    let mut etag = format!("{}", Utc::now().timestamp());

    // Store each session key
    for (session_id, session_data) in request.sessions {
        let backup_id = format!("{}:{}:{}", user_id, room_id, session_id);
        
        let _: Option<Value> = state.db
            .create(("room_key_backups", backup_id))
            .content(json!({
                "user_id": user_id,
                "room_id": room_id,
                "session_id": session_id,
                "version": backup_version,
                "first_message_index": session_data.first_message_index,
                "forwarded_count": session_data.forwarded_count,
                "is_verified": session_data.is_verified,
                "session_data": session_data.session_data,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store room key backup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        count += 1;
    }

    // Update backup version count and etag
    let update_query = "
        UPDATE backup_versions 
        SET count = count + $new_count, etag = $etag, updated_at = $updated_at
        WHERE user_id = $user_id AND version = $version
    ";
    
    let _update_result = state.db
        .query(update_query)
        .bind(("user_id", &user_id))
        .bind(("version", &backup_version))
        .bind(("new_count", count))
        .bind(("etag", &etag))
        .bind(("updated_at", Utc::now()))
        .await
        .map_err(|e| {
            error!("Failed to update backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

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
    let version_query = "SELECT version FROM backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let mut version_response = state.db
        .query(version_query)
        .bind(("user_id", &user_id))
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let version: Option<String> = version_response.take(0)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let backup_version = version.ok_or(StatusCode::NOT_FOUND)?;

    // Get all backed up sessions for this room
    let query = "SELECT * FROM room_key_backups WHERE user_id = $user_id AND room_id = $room_id AND version = $version";
    let mut response = state.db
        .query(query)
        .bind(("user_id", &user_id))
        .bind(("room_id", &room_id))
        .bind(("version", &backup_version))
        .await
        .map_err(|e| {
            error!("Failed to get room key backups: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let backups: Vec<Value> = response.take(0)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
    let version_query = "SELECT version FROM backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let mut version_response = state.db
        .query(version_query)
        .bind(("user_id", &user_id))
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let version: Option<String> = version_response.take(0)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let backup_version = version.ok_or(StatusCode::NOT_FOUND)?;

    // Delete all room key backups for this room
    let delete_query = "DELETE FROM room_key_backups WHERE user_id = $user_id AND room_id = $room_id AND version = $version";
    let _delete_result = state.db
        .query(delete_query)
        .bind(("user_id", &user_id))
        .bind(("room_id", &room_id))
        .bind(("version", &backup_version))
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