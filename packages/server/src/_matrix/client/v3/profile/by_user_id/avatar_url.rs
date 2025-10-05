use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use matryx_surrealdb::repository::ProfileManagementService;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::AppState;

#[derive(Serialize)]
pub struct AvatarUrlResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
pub struct SetAvatarUrlRequest {
    pub avatar_url: Option<String>,
}

fn validate_avatar_url(avatar_url: &str) -> Result<(), &'static str> {
    // Matrix spec: avatar_url must be a valid MXC URI or HTTP(S) URL
    if avatar_url.starts_with("mxc://") {
        // MXC URI format: mxc://{server-name}/{media-id}
        let parts: Vec<&str> = avatar_url.strip_prefix("mxc://").unwrap_or("").split('/').collect();

        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err("Invalid MXC URI format");
        }

        // Validate server name and media ID
        if parts[0].len() > 255 || parts[1].len() > 255 {
            return Err("MXC URI components too long");
        }

        Ok(())
    } else {
        // HTTP(S) URL validation
        match Url::parse(avatar_url) {
            Ok(url) => {
                if url.scheme() == "http" || url.scheme() == "https" {
                    Ok(())
                } else {
                    Err("Avatar URL must use HTTP or HTTPS scheme")
                }
            },
            Err(_) => Err("Invalid URL format"),
        }
    }
}

pub async fn get_avatar_url(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<AvatarUrlResponse>, StatusCode> {
    let profile_service = ProfileManagementService::new(state.db.clone());

    // Get user profile using ProfileManagementService
    match profile_service.get_user_profile(&user_id, &user_id).await {
        Ok(profile) => Ok(Json(AvatarUrlResponse { avatar_url: profile.avatar_url })),
        Err(_) => {
            // If no profile exists, return null avatar URL
            Ok(Json(AvatarUrlResponse { avatar_url: None }))
        },
    }
}

pub async fn set_avatar_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<SetAvatarUrlRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate avatar URL if provided
    if let Some(ref avatar_url) = request.avatar_url
        && !avatar_url.is_empty()
    {
        validate_avatar_url(avatar_url).map_err(|_| StatusCode::BAD_REQUEST)?;
    }

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Update avatar URL using ProfileManagementService
    match profile_service.update_avatar_url(&user_id, request.avatar_url).await {
        Ok(()) => Ok(Json(serde_json::json!({}))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// HTTP method handlers for main.rs routing
pub use get_avatar_url as get;
pub use set_avatar_url as put;
