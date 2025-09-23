use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    body::Body,
};
use serde::Deserialize;
use tracing::{debug, error, warn};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::state::AppState;
use crate::auth::verify_x_matrix_auth;
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    room::RoomRepository,
    membership::MembershipRepository,
};
use std::sync::Arc;

/// Query parameters for media download
#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    timeout_ms: Option<u64>,
}

/// GET /_matrix/federation/v1/media/download/{mediaId}
///
/// Downloads media content from the local server for federation.
/// This endpoint requires X-Matrix authentication and serves media
/// that was previously uploaded to this homeserver.
pub async fn get(
    State(state): State<AppState>,
    Path(media_id): Path<String>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    // Verify X-Matrix authentication
    let auth_result = verify_x_matrix_auth(&headers, &state.homeserver_name, state.event_signer.get_default_key_id()).await;
    let _x_matrix_auth = auth_result.map_err(|e| {
        warn!("X-Matrix authentication failed for media download: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    debug!("Federation media download request for media_id: {}", media_id);

    // Validate media_id format
    if media_id.is_empty() || media_id.len() > 255 {
        warn!("Invalid media_id format: {}", media_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    
    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Handle federation media request using MediaService
    let media_response = media_service
        .handle_federation_media_request(
            &media_id,
            &state.homeserver_name,
            &_x_matrix_auth.origin,
        )
        .await
        .map_err(|e| {
            debug!("Media not found or access denied: {}", e);
            StatusCode::NOT_FOUND
        })?;

    debug!(
        "Serving federation media: id={}, type={}, size={}",
        media_id, media_response.content_type, media_response.content_length
    );

    // Create response body from content
    let body = Body::from(media_response.content);

    // Build response with appropriate headers and security headers
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", media_response.content_type)
        .header("Content-Length", media_response.content_length.to_string())
        .header("Cache-Control", "public, max-age=31536000, immutable") // 1 year cache
        .header("Content-Security-Policy", 
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin");

    // Add CORS headers for federation
    response = response
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .header("Access-Control-Allow-Headers", "Authorization, Content-Type");

    let response = response.body(body).map_err(|e| {
        error!("Failed to build media response: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    debug!("Successfully serving media: {}", media_id);
    Ok(response)
}