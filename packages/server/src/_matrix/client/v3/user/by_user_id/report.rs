use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    AppState,
};

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
    let token_info = state.session_service
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

    if let Some(score) = request.score {
        if score < -100 || score > 0 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Prevent self-reporting
    if token_info.user_id == reported_user_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Verify reported user exists
    let user_exists_query = "SELECT user_id FROM users WHERE user_id = $user_id";
    let mut user_params = HashMap::new();
    user_params.insert("user_id".to_string(), Value::String(reported_user_id.clone()));

    let user_result = state.database
        .query(user_exists_query, Some(user_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_exists = user_result
        .first()
        .and_then(|rows| rows.first())
        .is_some();

    if !user_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    // Check if reporter has interacted with reported user (shared rooms)
    let interaction_query = r#"
        SELECT count() as shared_rooms
        FROM room_members rm1
        JOIN room_members rm2 ON rm1.room_id = rm2.room_id
        WHERE rm1.user_id = $reporter_user_id 
        AND rm2.user_id = $reported_user_id
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
    "#;

    let mut interaction_params = HashMap::new();
    interaction_params.insert("reporter_user_id".to_string(), Value::String(token_info.user_id.clone()));
    interaction_params.insert("reported_user_id".to_string(), Value::String(reported_user_id.clone()));

    let interaction_result = state.database
        .query(interaction_query, Some(interaction_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let has_interaction = interaction_result
        .first()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("shared_rooms"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) > 0;

    if !has_interaction {
        return Err(StatusCode::FORBIDDEN);
    }

    // Create report
    let report_id = Uuid::new_v4().to_string();
    
    let create_report_query = r#"
        CREATE content_reports SET
            id = $id,
            reporter_user_id = $reporter_user_id,
            reported_user_id = $reported_user_id,
            reason = $reason,
            score = $score,
            status = 'pending',
            created_at = time::now()
    "#;

    let mut report_params = HashMap::new();
    report_params.insert("id".to_string(), Value::String(report_id));
    report_params.insert("reporter_user_id".to_string(), Value::String(token_info.user_id));
    report_params.insert("reported_user_id".to_string(), Value::String(reported_user_id));
    report_params.insert("reason".to_string(), Value::String(request.reason));
    report_params.insert("score".to_string(), 
        request.score.map(|s| Value::Number(serde_json::Number::from(s))).unwrap_or(Value::Null));

    state.database
        .query(create_report_query, Some(report_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // TODO: Notify moderators/administrators

    Ok(Json(ReportResponse {}))
}