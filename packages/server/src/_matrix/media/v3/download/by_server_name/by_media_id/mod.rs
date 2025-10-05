use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tracing::{debug, warn};

use crate::utils::response_helpers::{
    MediaContent, MultipartMediaResponse, build_multipart_media_response,
};
use crate::{AppState, error::MatrixError};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};

pub mod by_file_name;

/// Query parameters for media download
#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    timeout_ms: Option<u64>,
}

/// GET /_matrix/media/v3/download/{serverName}/{mediaId}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, MatrixError> {
    debug!("Client media download request for media_id: {} from server: {}", media_id, server_name);

    // Validate media_id format
    if media_id.is_empty() || media_id.len() > 255 {
        warn!("Invalid media_id format: {}", media_id);
        return Err(MatrixError::Unknown);
    }

    // Validate server_name format
    if server_name.is_empty() || server_name.len() > 255 {
        warn!("Invalid server_name format: {}", server_name);
        return Err(MatrixError::Unknown);
    }

    // Add timeout validation and application
    let timeout_ms = query.timeout_ms.unwrap_or(20000).min(120000); // Max 2 minutes
    let timeout_duration = Duration::from_millis(timeout_ms);

    // Create MediaService instance with federation support
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo)
        .with_federation_client(
            state.federation_media_client.clone(),
            state.homeserver_name.clone(),
        );

    // Apply timeout to MediaService call
    let download_result = tokio::time::timeout(
        timeout_duration,
        media_service.download_media(&media_id, &server_name, "anonymous"),
    )
    .await
    .map_err(|_| MatrixError::NotYetUploaded)? // Timeout = content not ready
    .map_err(|e| {
        debug!("Media service error: {}", e);
        MatrixError::from(e)
    })?;

    debug!(
        "Serving client media: id={}, type={}, size={}",
        media_id, download_result.content_type, download_result.content_length
    );

    // ✅ COMPLIANT: Multipart/mixed response
    let multipart_response = MultipartMediaResponse {
        metadata: serde_json::json!({}), // Empty object per spec
        content: MediaContent::Bytes {
            data: download_result.content,
            content_type: download_result.content_type,
            filename: download_result.filename,
        },
    };

    let response = build_multipart_media_response(multipart_response).map_err(|e| {
        debug!("Failed to build multipart response: {}", e);
        MatrixError::Unknown
    })?;

    debug!("Successfully serving media: {}", media_id);
    Ok(response)
}
