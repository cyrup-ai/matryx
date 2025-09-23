use crate::{AppState, auth::MatrixSessionService};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
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

    // Return content URI for this specific media
    Ok(Json(json!({
        "content_uri": format!("mxc://{}/{}", server_name, media_id)
    })))
}
