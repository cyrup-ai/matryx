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
pub struct ReportEventRequest {
    pub reason: String,
    pub score: Option<i32>, // -100 to 0 (most offensive)
}

#[derive(Serialize)]
pub struct ReportResponse {
    // Empty response per Matrix spec
}

/// POST /_matrix/client/v3/rooms/{roomId}/report/{eventId}
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((room_id, event_id)): Path<(String, String)>,
    Json(request): Json<ReportEventRequest>,
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

    // Verify user is member of the room
    let membership_query = "SELECT membership FROM room_members WHERE room_id = $room_id AND user_id = $user_id";
    let mut membership_params = HashMap::new();
    membership_params.insert("room_id".to_string(), Value::String(room_id.clone()));
    membership_params.insert("user_id".to_string(), Value::String(token_info.user_id.clone()));

    let membership_result = state.database
        .query(membership_query, Some(membership_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_member = membership_result
        .first()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("membership"))
        .and_then(|v| v.as_str())
        .map(|membership| membership == "join" || membership == "invite")
        .unwrap_or(false);

    if !is_member {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify event exists and get event details
    let event_query = "SELECT sender, type FROM events WHERE event_id = $event_id AND room_id = $room_id";
    let mut event_params = HashMap::new();
    event_params.insert("event_id".to_string(), Value::String(event_id.clone()));
    event_params.insert("room_id".to_string(), Value::String(room_id.clone()));

    let event_result = state.database
        .query(event_query, Some(event_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let event_data = event_result
        .first()
        .and_then(|rows| rows.first())
        .ok_or(StatusCode::NOT_FOUND)?;

    let event_sender = event_data
        .get("sender")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prevent self-reporting
    if token_info.user_id == event_sender {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create report
    let report_id = Uuid::new_v4().to_string();
    
    let create_report_query = r#"
        CREATE content_reports SET
            id = $id,
            reporter_user_id = $reporter_user_id,
            reported_user_id = $reported_user_id,
            room_id = $room_id,
            event_id = $event_id,
            reason = $reason,
            score = $score,
            status = 'pending',
            created_at = time::now()
    "#;

    let mut report_params = HashMap::new();
    report_params.insert("id".to_string(), Value::String(report_id));
    report_params.insert("reporter_user_id".to_string(), Value::String(token_info.user_id));
    report_params.insert("reported_user_id".to_string(), Value::String(event_sender.to_string()));
    report_params.insert("room_id".to_string(), Value::String(room_id));
    report_params.insert("event_id".to_string(), Value::String(event_id));
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