use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::{Value, json};
use tracing::error;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::{PublicRoomsRepository, RoomDirectoryVisibility, PowerLevelsRepository};

/// GET /_matrix/client/v3/directory/list/room/{roomId}
pub async fn get(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // Use PublicRoomsRepository to get room directory visibility
    let public_rooms_repo = PublicRoomsRepository::new(state.db.clone());

    match public_rooms_repo.get_room_directory_visibility(&room_id).await {
        Ok(Some(RoomDirectoryVisibility::Public)) => Ok(Json(json!({
            "visibility": "public"
        }))),
        Ok(Some(RoomDirectoryVisibility::Private)) | Ok(None) => Ok(Json(json!({
            "visibility": "private"
        }))),
        Err(e) => {
            error!("Failed to get room directory visibility: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

/// PUT /_matrix/client/v3/directory/list/room/{roomId}
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Authenticate user
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Authentication failed: {}", e);
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

    // Parse visibility from payload
    let visibility_str = payload.get("visibility").and_then(|v| v.as_str()).unwrap_or("private");

    // Check user authorization to change directory visibility (requires moderator power level)
    let power_levels_repo = PowerLevelsRepository::new(state.db.clone());
    let user_power_level = match power_levels_repo.get_user_power_level(&room_id, &user_id).await {
        Ok(level) => level,
        Err(e) => {
            error!("Failed to get user power level: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Directory visibility changes are room state changes requiring moderator power level
    const REQUIRED_POWER_LEVEL: i32 = 50;
    if user_power_level < REQUIRED_POWER_LEVEL {
        tracing::warn!(
            "User {} denied: power level {} < required {} for room {} directory visibility change",
            user_id, user_power_level, REQUIRED_POWER_LEVEL, room_id
        );
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "You don't have permission to change room directory visibility"
        })));
    }

    // Authorization successful - log and proceed
    tracing::info!(
        "User {} authorized to change room {} directory visibility to {} (power level: {})",
        user_id, room_id, visibility_str, user_power_level
    );

    let visibility = match visibility_str {
        "public" => RoomDirectoryVisibility::Public,
        "private" => RoomDirectoryVisibility::Private,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Use PublicRoomsRepository to set room directory visibility
    let public_rooms_repo = PublicRoomsRepository::new(state.db.clone());

    match visibility {
        RoomDirectoryVisibility::Public => {
            public_rooms_repo.add_room_to_directory(&room_id, visibility).await
        },
        RoomDirectoryVisibility::Private => {
            public_rooms_repo.remove_room_from_directory(&room_id).await
        },
    }
    .map_err(|e| {
        error!("Failed to update room directory visibility: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({})))
}
