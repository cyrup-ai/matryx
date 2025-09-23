use crate::AppState;
use crate::auth::verify_x_matrix_auth;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;
use tracing::{debug, warn};

/// GET /_matrix/federation/v1/media/download/{serverName}/{mediaId}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response<Body>, StatusCode> {
    // Verify X-Matrix authentication
    let auth_result = verify_x_matrix_auth(
        &headers,
        &state.homeserver_name,
        state.event_signer.get_default_key_id(),
    )
    .await;
    let _x_matrix_auth = auth_result.map_err(|e| {
        warn!("X-Matrix authentication failed for media download: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    debug!("Federation media download request for server: {}, media_id: {}", server_name, media_id);

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Handle federation media request using MediaService
    let media_response = media_service
        .handle_federation_media_request(
            &media_id,
            &server_name,
            _x_matrix_auth.server_name().unwrap_or(&server_name),
        )
        .await
        .map_err(|e| {
            debug!("Media not found or access denied: {}", e);
            StatusCode::NOT_FOUND
        })?;

    // Create response body from content
    let body = Body::from(media_response.content);

    // Build response with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", media_response.content_type)
        .header("Content-Length", media_response.content_length.to_string())
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}
