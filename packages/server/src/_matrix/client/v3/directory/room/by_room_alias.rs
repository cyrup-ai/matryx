use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::error;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::MembershipState;
use matryx_surrealdb::repository::{
    MembershipRepository,
    RoomAliasRepository,
    PowerLevelsRepository,
    PowerLevelAction
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
    // Use MembershipRepository to check if user is member of the room
    let membership_repo = MembershipRepository::new(state.db.clone());
    let membership = membership_repo
        .get_membership(room_id, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match membership {
        Some(membership) if membership.membership == MembershipState::Join => {
            // Check if user has power level to manage aliases
            let power_levels_repo = PowerLevelsRepository::new(state.db.clone());
            let can_manage_aliases = power_levels_repo
                .can_user_perform_action(
                    room_id,
                    user_id,
                    PowerLevelAction::SendState("m.room.canonical_alias".to_string()),
                )
                .await
                .map_err(|e| {
                    error!("Failed to check power levels for user {} in room {}: {}", user_id, room_id, e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            if !can_manage_aliases {
                error!(
                    "User {} lacks power level to manage aliases in room {}",
                    user_id, room_id
                );
                return Err(StatusCode::FORBIDDEN);
            }

            // Validate alias domain permissions
            // According to Matrix spec, users should only be able to manage aliases
            // on their own homeserver domain
            let homeserver_name = &state.homeserver_name;
            if let Some(alias_domain) = alias.split(':').nth(1)
                && alias_domain != homeserver_name
            {
                error!(
                    "User {} attempted to manage alias {} on foreign domain {}",
                    user_id, alias, alias_domain
                );
                return Err(StatusCode::FORBIDDEN);
            }

            Ok(())
        },
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

    // Use RoomAliasRepository to resolve alias and get room_id for permission check
    let room_alias_repo = RoomAliasRepository::new(state.db.clone());
    let alias_info = room_alias_repo
        .resolve_alias(&room_alias)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Validate permissions
    validate_alias_permissions(&state, &user_id, &room_alias, &alias_info.room_id).await?;

    // Delete the alias using repository
    room_alias_repo
        .delete_alias(&room_alias)
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

    // Use RoomAliasRepository to resolve alias
    let room_alias_repo = RoomAliasRepository::new(state.db.clone());

    match room_alias_repo.resolve_alias(&room_alias).await {
        Ok(Some(alias_info)) => {
            // Create proper room alias record with server information
            let alias_record = RoomAliasRecord {
                room_id: alias_info.room_id.clone(),
                servers: vec![state.homeserver_name.clone()],
            };

            // Use the alias record to create response
            Ok(Json(RoomAliasResponse {
                room_id: alias_record.room_id,
                servers: alias_record.servers,
            }))
        },
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to resolve room alias: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
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

    // Create comprehensive room alias record with metadata
    let alias_record = RoomAlias {
        alias: room_alias.clone(),
        room_id: request.room_id.clone(),
        creator: user_id.clone(),
        created_at: chrono::Utc::now(),
        servers: vec![state.homeserver_name.clone()],
    };

    // Create alias using repository
    let room_alias_repo = RoomAliasRepository::new(state.db.clone());
    room_alias_repo
        .create_alias(&alias_record.alias, &alias_record.room_id, &alias_record.creator)
        .await
        .map_err(|e| {
            error!("Failed to create alias: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({})))
}
