use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use matryx_surrealdb::repository::ProfileManagementService;
use serde::Serialize;

use crate::{AppState, auth::MatrixSessionService};

#[derive(Serialize)]
pub struct WhoAmIResponse {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_guest: Option<bool>,
}

pub async fn whoami(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WhoAmIResponse>, StatusCode> {
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

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Get whoami info using ProfileManagementService
    match profile_service.get_whoami_info(&token_info.user_id).await {
        Ok(whoami_response) => {
            Ok(Json(WhoAmIResponse {
                user_id: whoami_response.user_id,
                device_id: Some(token_info.device_id),
                is_guest: whoami_response.is_guest,
            }))
        },
        Err(_) => {
            Ok(Json(WhoAmIResponse {
                user_id: token_info.user_id,
                device_id: Some(token_info.device_id),
                is_guest: Some(false),
            }))
        },
    }
}

// HTTP method handler for main.rs routing
pub use whoami as get;
