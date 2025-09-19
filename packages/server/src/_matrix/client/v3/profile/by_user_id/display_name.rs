use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

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
pub struct DisplayNameResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,
}

#[derive(Deserialize)]
pub struct SetDisplayNameRequest {
    pub displayname: Option<String>,
}

pub async fn get_display_name(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<DisplayNameResponse>, StatusCode> {
    // Query user profile from database
    let query = "SELECT display_name FROM user_profiles WHERE user_id = $user_id";
    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id.clone()));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(profiles) = result.first() {
        if let Some(profile_data) = profiles.first() {
            let display_name = profile_data.get("display_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            return Ok(Json(DisplayNameResponse {
                displayname: display_name,
            }));
        }
    }

    // If no profile exists, return null display name
    Ok(Json(DisplayNameResponse {
        displayname: None,
    }))
}

pub async fn set_display_name(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<SetDisplayNameRequest>,
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

    // Validate display name length (Matrix spec: max 256 characters)
    if let Some(ref display_name) = request.displayname {
        if display_name.len() > 256 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Update or create user profile
    let query = r#"
        UPDATE user_profiles SET 
            display_name = $display_name,
            updated_at = time::now()
        WHERE user_id = $user_id
        ELSE CREATE user_profiles SET
            user_id = $user_id,
            display_name = $display_name,
            created_at = time::now(),
            updated_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id));
    params.insert("display_name".to_string(), 
        request.displayname.map(Value::String).unwrap_or(Value::Null));

    state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({})))
}