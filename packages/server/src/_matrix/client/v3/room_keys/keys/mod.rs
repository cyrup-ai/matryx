use crate::_matrix::client::v3::room_keys::version::{
    BackupError, BackupVersionQuery, generate_backup_etag, get_backup_count,
    store_room_key, validate_backup_version,
};
use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{error, info};

/// DELETE /_matrix/client/v3/room_keys/keys
pub async fn delete() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "count": 0,
        "etag": "0"
    })))
}

/// GET /_matrix/client/v3/room_keys/keys
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "rooms": {}
    })))
}

/// PUT /_matrix/client/v3/room_keys/keys
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<BackupVersionQuery>,
    Json(rooms_data): Json<HashMap<String, HashMap<String, Value>>>,
) -> Result<Json<Value>, StatusCode> {
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

    let version = params.version.ok_or(StatusCode::BAD_REQUEST)?;

    // Validate backup version using production validation logic
    match validate_backup_version(&state.db, &user_id, &version).await {
        Ok(backup_version) => {
            let mut total_stored = 0;

            // Store keys for all rooms and sessions
            for (room_id, sessions_data) in rooms_data {
                for (session_id, key_data) in sessions_data {
                    match store_room_key(
                        &state.db,
                        &user_id,
                        &version,
                        &room_id,
                        &session_id,
                        &key_data,
                    )
                    .await
                    {
                        Ok(_) => total_stored += 1,
                        Err(e) => {
                            error!("Failed to store room key {}/{}: {}", room_id, session_id, e);
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        },
                    }
                }
            }

            info!(
                "Stored {} total room keys (user={}, version={})",
                total_stored, user_id, version
            );

            Ok(Json(serde_json::json!({
                "etag": format!("{}:{}", generate_backup_etag(&user_id, &version), backup_version.created_at),
                "count": get_backup_count(&state.db, &user_id, &version)
                    .await
                    .unwrap_or(total_stored)
            })))
        },
        Err(BackupError::InvalidVersion) => {
            error!("Invalid or non-existent backup version: {}", version);
            Err(StatusCode::NOT_FOUND)
        },
        Err(e) => {
            error!("Backup validation failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

pub mod by_room_id;
