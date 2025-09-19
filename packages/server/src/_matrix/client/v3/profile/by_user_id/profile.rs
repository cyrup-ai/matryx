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
pub struct ProfileResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

pub async fn get_profile(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<ProfileResponse>, StatusCode> {
    // Query user profile from database
    let query = "SELECT * FROM user_profiles WHERE user_id = $user_id";
    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id.clone()));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(profiles) = result.first() {
        if let Some(profile_data) = profiles.first() {
            let profile: UserProfile = serde_json::from_value(profile_data.clone())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            return Ok(Json(ProfileResponse {
                displayname: profile.display_name,
                avatar_url: profile.avatar_url,
            }));
        }
    }

    // If no profile exists, return empty profile
    Ok(Json(ProfileResponse {
        displayname: None,
        avatar_url: None,
    }))
}