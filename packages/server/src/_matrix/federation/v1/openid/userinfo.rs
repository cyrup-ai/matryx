use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_surrealdb::{AuthRepository, UserRepository};

/// Query parameters for OpenID userinfo request
#[derive(Debug, Deserialize)]
pub struct UserinfoQuery {
    /// The OpenID access token to get information about the owner for
    access_token: String,
}

/// OpenID userinfo response
#[derive(Debug, Serialize)]
pub struct UserinfoResponse {
    /// The Matrix User ID who generated the token
    sub: String,
}

/// OpenID error response
#[derive(Debug, Serialize)]
pub struct OpenIdError {
    errcode: String,
    error: String,
}

/// GET /_matrix/federation/v1/openid/userinfo
///
/// Exchanges an OpenID access token for information about the user who generated the token.
/// Currently this only exposes the Matrix User ID of the owner.
pub async fn get(
    State(state): State<AppState>,
    Query(params): Query<UserinfoQuery>,
) -> Result<Json<UserinfoResponse>, (StatusCode, Json<OpenIdError>)> {
    debug!("OpenID userinfo request for token: {}", params.access_token);

    // Validate access token format
    if params.access_token.is_empty() {
        warn!("Empty access token provided");
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(OpenIdError {
                errcode: "M_UNKNOWN_TOKEN".to_string(),
                error: "Access token unknown or expired".to_string(),
            }),
        ));
    }

    // Query database for OpenID token
    let auth_repo = AuthRepository::new(state.db.clone());
    let token = auth_repo
        .validate_openid_token(&params.access_token)
        .await
        .map_err(|e| {
            error!("Failed to query OpenID token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenIdError {
                    errcode: "M_UNKNOWN".to_string(),
                    error: "Internal server error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            warn!("OpenID token not found: {}", params.access_token);
            (
                StatusCode::UNAUTHORIZED,
                Json(OpenIdError {
                    errcode: "M_UNKNOWN_TOKEN".to_string(),
                    error: "Access token unknown or expired".to_string(),
                }),
            )
        })?;

    // Check if token has expired according to Matrix OpenID specification
    let current_timestamp = chrono::Utc::now().timestamp();
    if token.1 < current_timestamp {
        warn!("OpenID token expired at {} (current: {}): {}", 
              token.1, current_timestamp, params.access_token);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(OpenIdError {
                errcode: "M_UNKNOWN_TOKEN".to_string(),
                error: "Access token expired".to_string(),
            }),
        ));
    }

    // Verify user still exists
    let user_repo = Arc::new(UserRepository::new(state.db.clone()));
    let user = user_repo
        .get_by_id(&token.0)
        .await
        .map_err(|e| {
            error!("Failed to query user {}: {}", token.0, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenIdError {
                    errcode: "M_UNKNOWN".to_string(),
                    error: "Internal server error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            warn!("User {} not found for OpenID token", token.0);
            (
                StatusCode::UNAUTHORIZED,
                Json(OpenIdError {
                    errcode: "M_UNKNOWN_TOKEN".to_string(),
                    error: "Access token unknown or expired".to_string(),
                }),
            )
        })?;

    info!("OpenID userinfo request successful for user: {}", user.user_id);

    Ok(Json(UserinfoResponse { sub: user.user_id }))
}
