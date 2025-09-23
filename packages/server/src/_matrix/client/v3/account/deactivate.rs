use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::{AppState, auth::MatrixSessionService};
use matryx_surrealdb::repository::ProfileManagementService;

#[derive(Deserialize)]
pub struct DeactivateAccountRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Value>, // Authentication data for verification
    #[serde(default)]
    pub erase: bool, // Whether to erase all user data
}

#[derive(Serialize)]
pub struct DeactivateAccountResponse {
    pub id_server_unbind_result: String,
}

pub async fn deactivate_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeactivateAccountRequest>,
) -> Result<Json<DeactivateAccountResponse>, StatusCode> {
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

    // TODO: Validate auth data if provided (password confirmation, etc.)
    // For now, we'll proceed without additional auth validation

    let user_id = &token_info.user_id;

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Deactivate account using ProfileManagementService
    match profile_service.deactivate_account(user_id, request.erase).await {
        Ok(()) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    Ok(Json(DeactivateAccountResponse { id_server_unbind_result: "success".to_string() }))
}

// HTTP method handler for main.rs routing
pub use deactivate_account as post;
