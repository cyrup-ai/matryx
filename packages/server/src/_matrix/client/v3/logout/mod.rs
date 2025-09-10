use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::{RepositoryError, session::SessionRepository};

/// Matrix Client-Server API v1.11 Section 5.4.2
///
/// POST /_matrix/client/v3/logout
///
/// Invalidates the access token used to make the request, effectively logging out the current device.
/// The device will no longer be able to use that access token for authorization.
///
/// This endpoint requires authentication and will return an empty JSON object on success.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(_request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Logout failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let (user_id, device_id, access_token) = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Logout failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            (token_info.user_id.clone(), token_info.device_id.clone(), token_info.token.clone())
        },
        MatrixAuth::Server(_) => {
            warn!("Logout failed - server authentication not allowed for logout");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Logout failed - anonymous authentication not allowed for logout");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing logout request for user: {} device: {} from: {}", user_id, device_id, addr);

    // Create session repository for token invalidation
    let session_repo = SessionRepository::new(state.db.clone());

    // Invalidate the current access token
    if let Err(e) = session_repo.invalidate_token(&access_token).await {
        error!("Failed to invalidate access token during logout: {}", e);
        return match e {
            RepositoryError::NotFound { .. } => {
                info!(
                    "Access token already invalidated for user: {} device: {}",
                    user_id, device_id
                );
                Ok(Json(json!({})))
            },
            _ => {
                error!("Database error during logout token invalidation: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            },
        };
    }

    // Log logout event for security monitoring
    info!(
        "Security event: logout for user: {} device: {} from ip: {} at {}",
        user_id,
        device_id,
        addr,
        chrono::Utc::now().timestamp()
    );

    let duration = start_time.elapsed();
    info!(
        "User logout completed successfully for user: {} device: {} duration: {:?}",
        user_id, device_id, duration
    );

    // Return empty JSON object as per Matrix spec
    Ok(Json(json!({})))
}

pub mod all;
