use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

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
    let version_query = "SELECT version FROM backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let mut version_response = state
        .db
        .query(version_query)
        .bind(("user_id", user_id.clone()))
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let version: Option<String> =
        version_response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let backup_version = version.ok_or(StatusCode::NOT_FOUND)?;

    // Delete specific session key backup
    let backup_id = format!("{}:{}:{}", user_id, room_id, session_id);
    let delete_query = "DELETE FROM room_key_backups WHERE id = $backup_id AND version = $version";
    let mut delete_result = state
        .db
        .query(delete_query)
        .bind(("backup_id", backup_id))
        .bind(("version", backup_version))
        .await
        .map_err(|e| {
            error!("Failed to delete session key backup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let deleted: Option<Value> =
        delete_result.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if deleted.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

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
    let version_query = "SELECT version FROM backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let mut version_response = state
        .db
        .query(version_query)
        .bind(("user_id", user_id.clone()))
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let version: Option<String> =
        version_response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let backup_version = version.ok_or(StatusCode::NOT_FOUND)?;

    // Get specific session key backup
    let backup_id = format!("{}:{}:{}", user_id, room_id, session_id);
    let query = "SELECT * FROM room_key_backups WHERE id = $backup_id AND version = $version";
    let mut response = state
        .db
        .query(query)
        .bind(("backup_id", backup_id))
        .bind(("version", backup_version))
        .await
        .map_err(|e| {
            error!("Failed to get session key backup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let backup: Option<Value> = response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(backup_data) = backup {
        Ok(Json(json!({
            "first_message_index": backup_data.get("first_message_index").unwrap_or(&json!(0)),
            "forwarded_count": backup_data.get("forwarded_count").unwrap_or(&json!(0)),
            "is_verified": backup_data.get("is_verified").unwrap_or(&json!(false)),
            "session_data": backup_data.get("session_data").unwrap_or(&json!({}))
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
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
    let version_query = "SELECT version FROM backup_versions WHERE user_id = $user_id ORDER BY created_at DESC LIMIT 1";
    let mut version_response = state
        .db
        .query(version_query)
        .bind(("user_id", user_id.clone()))
        .await
        .map_err(|e| {
            error!("Failed to get backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let version: Option<String> =
        version_response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let backup_version = version.ok_or(StatusCode::PRECONDITION_FAILED)?;

    // Store session key backup
    let backup_id = format!("{}:{}:{}", user_id, room_id, session_id);
    let _: Option<Value> = state
        .db
        .create(("room_key_backups", backup_id))
        .content(json!({
            "user_id": user_id,
            "room_id": room_id,
            "session_id": session_id,
            "version": backup_version,
            "first_message_index": request.first_message_index,
            "forwarded_count": request.forwarded_count,
            "is_verified": request.is_verified,
            "session_data": request.session_data,
            "created_at": Utc::now()
        }))
        .await
        .map_err(|e| {
            error!("Failed to store session key backup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Update backup version count and etag
    let etag = format!("{}", Utc::now().timestamp());
    let update_query = "
        UPDATE backup_versions 
        SET count = count + 1, etag = $etag, updated_at = $updated_at
        WHERE user_id = $user_id AND version = $version
    ";

    let _update_result = state
        .db
        .query(update_query)
        .bind(("user_id", user_id.clone()))
        .bind(("version", backup_version.clone()))
        .bind(("etag", etag.clone()))
        .bind(("updated_at", Utc::now()))
        .await
        .map_err(|e| {
            error!("Failed to update backup version: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Session key backup stored: user={} room={} session={}", user_id, room_id, session_id);

    Ok(Json(json!({
        "count": 1,
        "etag": etag
    })))
}
