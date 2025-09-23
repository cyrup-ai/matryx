use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{error, info};

use crate::auth::extract_matrix_auth;
use crate::state::AppState;
use matryx_surrealdb::repository::notification::NotificationRepository;

#[derive(Deserialize)]
pub struct NotificationQuery {
    from: Option<String>,
    limit: Option<u32>,
    only: Option<String>,
}

/// GET /_matrix/client/v3/notifications
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<NotificationQuery>,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = match auth {
        crate::auth::MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id
        },
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    info!("Getting notifications for user: {}", user_id);

    // Create notification repository
    let notification_repo = NotificationRepository::new(state.db.clone());

    // Get user notifications
    let notifications_response = match notification_repo
        .get_user_notifications(&user_id, query.from.as_deref(), query.limit)
        .await
    {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to get notifications for user {}: {}", user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Convert to Matrix API format
    let notifications_json: Vec<Value> = notifications_response
        .notifications
        .into_iter()
        .map(|n| {
            json!({
                "actions": n.actions,
                "event": n.content,
                "profile_tag": null,
                "read": n.read,
                "room_id": n.room_id,
                "ts": n.created_at.timestamp_millis()
            })
        })
        .collect();

    Ok(Json(json!({
        "notifications": notifications_json,
        "next_token": notifications_response.next_token,
        "prev_token": notifications_response.prev_token
    })))
}
