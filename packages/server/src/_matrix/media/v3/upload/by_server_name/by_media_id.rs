use crate::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};
use serde_json::{Value, json};
use std::sync::Arc;

/// PUT /_matrix/media/v3/upload/{serverName}/{mediaId}
pub async fn put(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
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

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Validate media access permissions
    if !media_service
        .validate_media_access(&media_id, &server_name, &token_info.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }

    // Process the payload for media metadata updates
    // The payload typically contains media information like filename, content_type, etc.
    let filename = payload.get("filename").and_then(|f| f.as_str()).map(|s| s.to_string());

    let content_type = payload
        .get("content_type")
        .and_then(|ct| ct.as_str())
        .unwrap_or("application/octet-stream");

    // Update media metadata if provided in payload
    if let Some(ref name) = filename
        && let Err(e) = media_service
            .update_media_metadata(&media_id, &server_name, name, content_type)
            .await
    {
        tracing::warn!("Failed to update media metadata for {}/{}: {}", server_name, media_id, e);
        // Continue execution - metadata update failure shouldn't block the response
    }

    // Validate that the media exists after processing payload
    let media_exists = media_service
        .media_exists(&media_id, &server_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !media_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    // Return content URI for this specific media
    Ok(Json(json!({
        "content_uri": format!("mxc://{}/{}", server_name, media_id)
    })))
}
