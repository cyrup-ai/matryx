use axum::{Json, extract::State, http::StatusCode};

use crate::auth::refresh_token::{RefreshTokenRequest, RefreshTokenResponse, RefreshTokenService};
use crate::state::AppState;
use matryx_surrealdb::repository::auth::AuthRepository;

/// POST /_matrix/client/v3/refresh
/// Refresh an access token using a refresh token
pub async fn post(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<RefreshTokenResponse>, StatusCode> {
    // Create refresh token service
    let auth_repo = AuthRepository::new(state.db.clone());
    let refresh_service = RefreshTokenService::new(auth_repo, state.session_service);

    // Process refresh token request
    match refresh_service.refresh_tokens(&request.refresh_token).await {
        Ok(response) => Ok(Json(response)),
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}
