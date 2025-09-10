use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{AppState, auth::MatrixAuthError};
use matryx_surrealdb::repository::{RepositoryError, session::SessionRepository};

/// Matrix Client-Server API v1.11 Section 5.4.3 refresh token request
#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Matrix Client-Server API v1.11 Section 5.4.3 refresh token response
#[derive(Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub expires_in_ms: u64,
}

/// Matrix Client-Server API v1.11 Section 5.4.3
///
/// POST /_matrix/client/v3/refresh
///
/// Refresh an access token using a refresh token. This allows clients to obtain
/// a new access token when the current one expires, without requiring the user
/// to re-authenticate.
///
/// The refresh token provided must be valid and not expired. Upon successful
/// refresh, both a new access token and refresh token are returned, and the
/// old tokens are invalidated atomically.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    info!("Processing token refresh request from: {}", addr);

    // Validate refresh token format
    if request.refresh_token.is_empty() {
        warn!("Refresh token request failed - empty refresh token from: {}", addr);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Log the request for security monitoring (without exposing token)
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());

    // Use session service to validate and refresh token atomically
    let (new_access_token, new_refresh_token) =
        match state.session_service.refresh_token(&request.refresh_token).await {
            Ok((access, refresh)) => (access, refresh),
            Err(e) => {
                let status = match e {
                    MatrixAuthError::UnknownToken => {
                        warn!("Token refresh failed - unknown refresh token from: {}", addr);
                        StatusCode::UNAUTHORIZED
                    },
                    MatrixAuthError::SessionExpired => {
                        warn!("Token refresh failed - expired refresh token from: {}", addr);
                        StatusCode::UNAUTHORIZED
                    },
                    MatrixAuthError::JwtError(jwt_err) => {
                        warn!(
                            "Token refresh failed - JWT validation error: {} from: {}",
                            jwt_err, addr
                        );
                        StatusCode::UNAUTHORIZED
                    },
                    MatrixAuthError::DatabaseError(db_err) => {
                        error!("Token refresh failed - database error: {} from: {}", db_err, addr);
                        StatusCode::INTERNAL_SERVER_ERROR
                    },
                    _ => {
                        error!("Token refresh failed - unexpected error: {} from: {}", e, addr);
                        StatusCode::INTERNAL_SERVER_ERROR
                    },
                };
                return Err(status);
            },
        };

    // Create session repository for atomic token operations
    let session_repo = SessionRepository::new(state.db.clone());

    // Invalidate the old refresh token atomically (security best practice)
    if let Err(e) = session_repo.invalidate_token(&request.refresh_token).await {
        match e {
            RepositoryError::NotFound { .. } => {
                // Token already invalidated, which is fine
                info!("Refresh token already invalidated during refresh from: {}", addr);
            },
            _ => {
                error!(
                    "Failed to invalidate old refresh token during refresh: {} from: {}",
                    e, addr
                );
                // Continue anyway - new tokens are already generated
            },
        }
    }

    let duration = start_time.elapsed();
    info!("Token refresh completed successfully from: {} duration: {:?}", addr, duration);

    // Log refresh event for security monitoring
    info!(
        "Security event: token_refresh from ip: {} user_agent: {:?} at {}",
        addr,
        user_agent,
        chrono::Utc::now().timestamp()
    );

    // Return new tokens with standard expiration (1 hour for access token)
    let response = RefreshResponse {
        access_token: new_access_token,
        refresh_token: Some(new_refresh_token),
        expires_in_ms: 3600000, // 1 hour in milliseconds
    };

    Ok(Json(response))
}
