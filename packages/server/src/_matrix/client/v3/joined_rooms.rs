use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;
use std::sync::Arc;
use tracing::error;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::MembershipRepository;

#[derive(Serialize)]
pub struct JoinedRoomsResponse {
    joined_rooms: Vec<String>,
}

/// GET /_matrix/client/v3/joined_rooms
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<JoinedRoomsResponse>, StatusCode> {
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

    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let joined_rooms = membership_repo.get_joined_rooms_for_user(&user_id).await.map_err(|e| {
        error!("Failed to get joined rooms: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(JoinedRoomsResponse { joined_rooms }))
}
