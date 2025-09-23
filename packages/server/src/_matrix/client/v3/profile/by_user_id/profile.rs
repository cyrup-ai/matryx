use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use matryx_surrealdb::repository::ProfileManagementService;

use crate::{
    auth::MatrixSessionService,
    AppState,
};



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
    let profile_service = ProfileManagementService::new(state.db.clone());
    
    // Get user profile using ProfileManagementService
    match profile_service.get_user_profile(&user_id, &user_id).await {
        Ok(profile) => Ok(Json(ProfileResponse {
            displayname: profile.displayname,
            avatar_url: profile.avatar_url,
        })),
        Err(_) => {
            // If no profile exists, return empty profile
            Ok(Json(ProfileResponse {
                displayname: None,
                avatar_url: None,
            }))
        }
    }
}