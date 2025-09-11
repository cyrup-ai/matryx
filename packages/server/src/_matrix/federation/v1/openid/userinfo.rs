use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_surrealdb::repository::UserRepository;

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
    let query = "
        SELECT user_id, expires_at
        FROM openid_token
        WHERE access_token = $access_token
        AND expires_at > $current_time
        LIMIT 1
    ";

    let current_time = chrono::Utc::now().timestamp_millis();

    let mut response = state
        .db
        .query(query)
        .bind(("access_token", params.access_token.clone()))
        .bind(("current_time", current_time))
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
        })?;

    #[derive(serde::Deserialize)]
    struct TokenResult {
        user_id: String,
        expires_at: i64,
    }

    let token_result: Option<TokenResult> = response.take(0).map_err(|e| {
        error!("Failed to parse OpenID token result: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(OpenIdError {
                errcode: "M_UNKNOWN".to_string(),
                error: "Internal server error".to_string(),
            }),
        )
    })?;

    let token = token_result.ok_or_else(|| {
        warn!("OpenID token not found or expired: {}", params.access_token);
        (
            StatusCode::UNAUTHORIZED,
            Json(OpenIdError {
                errcode: "M_UNKNOWN_TOKEN".to_string(),
                error: "Access token unknown or expired".to_string(),
            }),
        )
    })?;

    // Verify user still exists
    let user_repo = Arc::new(UserRepository::new(state.db.clone()));
    let user = user_repo
        .get_by_id(&token.user_id)
        .await
        .map_err(|e| {
            error!("Failed to query user {}: {}", token.user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenIdError {
                    errcode: "M_UNKNOWN".to_string(),
                    error: "Internal server error".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            warn!("User {} not found for OpenID token", token.user_id);
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
