use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::error;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Serialize)]
pub struct JoinedRoomsResponse {
    joined_rooms: Vec<String>,
}

/// GET /_matrix/client/v3/joined_rooms
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<JoinedRoomsResponse>, StatusCode> {
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

    let query = "
        SELECT room_id 
        FROM membership 
        WHERE user_id = $user_id 
          AND membership = 'join'
        ORDER BY updated_at DESC
    ";

    let mut response = state.db.query(query).bind(("user_id", user_id)).await.map_err(|e| {
        error!("Database query failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let joined_rooms: Vec<String> = response.take(0).map_err(|e| {
        error!("Failed to parse query result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(JoinedRoomsResponse { joined_rooms }))
}
