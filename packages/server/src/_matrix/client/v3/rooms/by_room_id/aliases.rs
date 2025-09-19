use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;
use tracing::error;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Serialize)]
pub struct RoomAliasesResponse {
    aliases: Vec<String>,
}

async fn verify_room_membership(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<(), StatusCode> {
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

/// GET /_matrix/client/v3/rooms/{roomId}/aliases
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
) -> Result<Json<RoomAliasesResponse>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
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

    // Verify user is member of room
    verify_room_membership(&state, &room_id, &user_id).await?;

    let query = "
        SELECT alias 
        FROM room_aliases 
        WHERE room_id = $room_id 
          AND server_name = $server_name
        ORDER BY created_at ASC
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id))
        .bind(("server_name", state.homeserver_name.clone()))
        .await
        .map_err(|e| {
            error!("Database query failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let aliases: Vec<String> = response.take(0).map_err(|e| {
        error!("Failed to parse query result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(RoomAliasesResponse { aliases }))
}
