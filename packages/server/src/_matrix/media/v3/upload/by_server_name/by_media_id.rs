use crate::AppState;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};
use serde_json::json;
use std::sync::Arc;

/// PUT /_matrix/media/v3/upload/{serverName}/{mediaId}
///
/// Uploads binary content to a pending upload created via POST /_matrix/media/v1/create
pub async fn put(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
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

    // Validate server name matches our homeserver
    if server_name != state.homeserver_name {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get content type from header, default to application/octet-stream
    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream");

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Upload to pending reservation
    media_service
        .upload_to_pending(
            &media_id,
            &server_name,
            &token_info.user_id,
            &body,
            content_type,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to upload to pending: {:?}", e);
            match e {
                matryx_surrealdb::repository::media_service::MediaError::NotFound => {
                    StatusCode::NOT_FOUND // M_NOT_FOUND for expired or non-existent
                }
                matryx_surrealdb::repository::media_service::MediaError::AccessDenied(msg)
                    if msg.contains("M_FORBIDDEN") =>
                {
                    StatusCode::FORBIDDEN // M_FORBIDDEN for wrong user
                }
                matryx_surrealdb::repository::media_service::MediaError::InvalidOperation(msg)
                    if msg.contains("M_CANNOT_OVERWRITE_MEDIA") =>
                {
                    StatusCode::CONFLICT // 409 for already uploaded
                }
                matryx_surrealdb::repository::media_service::MediaError::TooLarge => {
                    StatusCode::PAYLOAD_TOO_LARGE
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    // Return empty JSON object as per spec
    Ok(Json(json!({})))
}
