use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::AppState;
use matryx_surrealdb::repository::ProfileManagementService;

#[derive(Deserialize)]
pub struct ReportUserRequest {
    pub reason: String,
    pub score: Option<i32>, // -100 to 0 (most offensive)
}

#[derive(Serialize)]
pub struct ReportResponse {
    // Empty response per Matrix spec
}

/// POST /_matrix/client/v3/user/{userId}/report
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(reported_user_id): Path<String>,
    Json(request): Json<ReportUserRequest>,
) -> Result<Json<ReportResponse>, StatusCode> {
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

    // Validate request
    if request.reason.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if request.reason.len() > 1000 {
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Some(score) = request.score
        && !(-100..=0).contains(&score)
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Prevent self-reporting
    if token_info.user_id == reported_user_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Create report content with score if provided
    let content = request.score.map(|score| serde_json::json!({ "score": score }));

    // Report user using ProfileManagementService
    match profile_service
        .report_user(&token_info.user_id, &reported_user_id, &request.reason, content)
        .await
    {
        Ok(()) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    // Notify server administrators of the report
    if let Some(email_service) = &state.email_service {
        let admin_email = &state.config.admin_email;
        if let Err(e) = email_service.send_moderator_notification(
            admin_email,
            &token_info.user_id,
            &reported_user_id,
            &request.reason,
        ).await {
            warn!("Failed to send moderator notification email: {}", e);
            // Continue anyway - report is already created
        } else {
            info!("Moderator notification sent to {}", admin_email);
        }
    } else {
        warn!("Email service not available - moderator notification not sent");
    }

    Ok(Json(ReportResponse {}))
}
