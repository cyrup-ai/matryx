use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use matryx_surrealdb::repository::ProfileManagementService;

use crate::{
    auth::MatrixSessionService,
    AppState,
};



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
    let profile_service = ProfileManagementService::new(state.db.clone());
    
    // Get user profile which includes display name
    match profile_service.get_user_profile(&user_id, &user_id).await {
        Ok(profile) => Ok(Json(DisplayNameResponse {
            displayname: profile.displayname,
        })),
        Err(_) => {
            // If profile doesn't exist, return null display name
            Ok(Json(DisplayNameResponse {
                displayname: None,
            }))
        }
    }
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

    let profile_service = ProfileManagementService::new(state.db.clone());
    
    // Update display name using profile service
    match profile_service.update_display_name(&user_id, request.displayname).await {
        Ok(()) => Ok(Json(serde_json::json!({}))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}