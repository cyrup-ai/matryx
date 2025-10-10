//! POST /_matrix/media/v1/create endpoint
//!
//! Creates a pending upload reservation for async upload flow.
//! The client receives an mxc:// URI and can upload content to it later
//! using PUT /_matrix/media/v3/upload/{serverName}/{mediaId}

use crate::AppState;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use serde_json::json;
use std::sync::Arc;

/// POST /_matrix/media/v1/create
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Extract Bearer token from Authorization header
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

    // Extract server name from user ID
    let server_name = token_info
        .user_id
        .split(':')
        .nth(1)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Validate server name matches our homeserver
    if server_name != state.homeserver_name {
        return Err(StatusCode::FORBIDDEN);
    }

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Create pending upload reservation
    let (media_id, expires_at) = media_service
        .create_pending_upload(&token_info.user_id, server_name)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create pending upload: {:?}", e);
            match e {
                matryx_surrealdb::repository::media_service::MediaError::InvalidOperation(msg)
                    if msg.contains("M_LIMIT_EXCEEDED") =>
                {
                    StatusCode::TOO_MANY_REQUESTS
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    // Return mxc:// URI with expiration
    Ok(Json(json!({
        "content_uri": format!("mxc://{}/{}", server_name, media_id),
        "unused_expires_at": expires_at.timestamp_millis()
    })))
}
