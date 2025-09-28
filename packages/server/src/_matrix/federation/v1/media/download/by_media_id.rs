use axum::{
    extract::{Path, Query, Request, State},
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
use crate::error::MatrixError;
use crate::utils::request_helpers::extract_request_uri;
use crate::utils::response_helpers::{build_multipart_media_response, MultipartMediaResponse, MediaContent};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    room::RoomRepository,
    membership::MembershipRepository,
};
use std::sync::Arc;
use std::time::Duration;

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
    request: Request,
) -> Result<Response, MatrixError> {
    // Verify X-Matrix authentication using actual request URI (including query parameters)
    let uri = extract_request_uri(&request);
    let auth_result = verify_x_matrix_auth(
        &headers,
        &state.homeserver_name,
        "GET",
        uri,
        None, // No body for GET requests
        state.event_signer.get_signing_engine(),
    ).await;
    let _x_matrix_auth = auth_result.map_err(|e| {
        warn!("X-Matrix authentication failed for media download: {}", e);
        MatrixError::from(e)
    })?;

    debug!("Federation media download request for media_id: {}", media_id);

    // Validate media_id format
    if media_id.is_empty() || media_id.len() > 255 {
        warn!("Invalid media_id format: {}", media_id);
        return Err(MatrixError::Unknown);
    }

    // Add timeout validation and application
    let timeout_ms = query.timeout_ms.unwrap_or(20000).min(120000); // Max 2 minutes
    let timeout_duration = Duration::from_millis(timeout_ms);

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Apply timeout to MediaService call
    let media_response = tokio::time::timeout(
        timeout_duration,
        media_service.handle_federation_media_request(
            &media_id,
            &state.homeserver_name,
            &_x_matrix_auth.origin,
        )
    )
    .await
    .map_err(|_| MatrixError::NotYetUploaded)? // Timeout = content not ready
    .map_err(|e| {
        debug!("Media service error: {}", e);
        MatrixError::from(e)
    })?;

    debug!(
        "Serving federation media: id={}, type={}, size={}",
        media_id, media_response.content_type, media_response.content_length
    );

    // ✅ COMPLIANT: Multipart/mixed response
    let multipart_response = MultipartMediaResponse {
        metadata: serde_json::json!({}), // Empty object per spec
        content: MediaContent::Bytes {
            data: media_response.content,
            content_type: media_response.content_type,
            filename: None,
        },
    };

    let response = build_multipart_media_response(multipart_response)
        .map_err(|e| {
            error!("Failed to build multipart response: {}", e);
            MatrixError::Unknown
        })?;

    debug!("Successfully serving media: {}", media_id);
    Ok(response)
}