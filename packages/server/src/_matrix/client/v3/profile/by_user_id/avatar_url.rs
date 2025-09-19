use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use url::Url;

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    AppState,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct UserProfile {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

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
        let parts: Vec<&str> = avatar_url.strip_prefix("mxc://")
            .unwrap_or("")
            .split('/')
            .collect();
        
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
            }
            Err(_) => Err("Invalid URL format"),
        }
    }
}

pub async fn get_avatar_url(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<AvatarUrlResponse>, StatusCode> {
    // Query user profile from database
    let query = "SELECT avatar_url FROM user_profiles WHERE user_id = $user_id";
    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id.clone()));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(profiles) = result.first() {
        if let Some(profile_data) = profiles.first() {
            let avatar_url = profile_data.get("avatar_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            return Ok(Json(AvatarUrlResponse {
                avatar_url,
            }));
        }
    }

    // If no profile exists, return null avatar URL
    Ok(Json(AvatarUrlResponse {
        avatar_url: None,
    }))
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
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate avatar URL if provided
    if let Some(ref avatar_url) = request.avatar_url {
        if !avatar_url.is_empty() {
            validate_avatar_url(avatar_url)
                .map_err(|_| StatusCode::BAD_REQUEST)?;
        }
    }

    // Update or create user profile
    let query = r#"
        UPDATE user_profiles SET 
            avatar_url = $avatar_url,
            updated_at = time::now()
        WHERE user_id = $user_id
        ELSE CREATE user_profiles SET
            user_id = $user_id,
            avatar_url = $avatar_url,
            created_at = time::now(),
            updated_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id));
    params.insert("avatar_url".to_string(), 
        request.avatar_url.map(Value::String).unwrap_or(Value::Null));

    state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({})))
}