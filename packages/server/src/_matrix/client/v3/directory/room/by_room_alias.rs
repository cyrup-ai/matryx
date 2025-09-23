use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::error;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Serialize, Deserialize)]
pub struct RoomAliasResponse {
    room_id: String,
    servers: Vec<String>,
}

#[derive(Deserialize)]
pub struct CreateAliasRequest {
    room_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct RoomAlias {
    alias: String,
    room_id: String,
    creator: String,
    created_at: chrono::DateTime<Utc>,
    servers: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct RoomAliasRecord {
    room_id: String,
    servers: Vec<String>,
}

fn validate_alias_format(alias: &str) -> Result<(), StatusCode> {
    if !alias.starts_with('#') || !alias.contains(':') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

async fn validate_alias_permissions(
    state: &AppState,
    user_id: &str,
    alias: &str,
    room_id: &str,
) -> Result<(), StatusCode> {
    // Check if user is member of the room
    let query = "SELECT membership FROM membership WHERE room_id = $room_id AND user_id = $user_id";
    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("user_id", user_id.to_string()))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let membership: Option<String> =
        response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match membership.as_deref() {
        Some("join") => Ok(()),
        _ => Err(StatusCode::FORBIDDEN),
    }
}

/// DELETE /_matrix/client/v3/directory/room/{roomAlias}
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_alias): Path<String>,
) -> Result<Json<Value>, StatusCode> {
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

    validate_alias_format(&room_alias)?;

    // Check if alias exists and get room_id for permission check
    let query = "SELECT room_id FROM room_aliases WHERE alias = $alias";
    let mut response = state
        .db
        .query(query)
        .bind(("alias", room_alias.clone()))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let room_id: Option<String> =
        response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let room_id = room_id.ok_or(StatusCode::NOT_FOUND)?;

    // Validate permissions
    validate_alias_permissions(&state, &user_id, &room_alias, &room_id).await?;

    // Delete the alias
    let _: Option<RoomAlias> = state
        .db
        .delete(("room_aliases", &room_alias))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({})))
}

/// GET /_matrix/client/v3/directory/room/{roomAlias}
pub async fn get(
    State(state): State<AppState>,
    Path(room_alias): Path<String>,
) -> Result<Json<RoomAliasResponse>, StatusCode> {
    validate_alias_format(&room_alias)?;

    let query = "SELECT room_id, servers FROM room_aliases WHERE alias = $alias";
    let mut response = state.db.query(query).bind(("alias", room_alias)).await.map_err(|e| {
        error!("Database query failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let alias_record: Option<RoomAliasRecord> = response.take(0).map_err(|e| {
        error!("Failed to parse query result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match alias_record {
        Some(record) => {
            Ok(Json(RoomAliasResponse { room_id: record.room_id, servers: record.servers }))
        },
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// PUT /_matrix/client/v3/directory/room/{roomAlias}
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_alias): Path<String>,
    Json(request): Json<CreateAliasRequest>,
) -> Result<Json<Value>, StatusCode> {
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

    validate_alias_format(&room_alias)?;
    validate_alias_permissions(&state, &user_id, &room_alias, &request.room_id).await?;

    // Create alias record
    let alias_record = RoomAlias {
        alias: room_alias.clone(),
        room_id: request.room_id.clone(),
        creator: user_id,
        created_at: Utc::now(),
        servers: vec![state.homeserver_name.clone()],
    };

    let _: Option<RoomAlias> = state
        .db
        .create(("room_aliases", &room_alias))
        .content(alias_record)
        .await
        .map_err(|e| {
            error!("Failed to create alias: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({})))
}
