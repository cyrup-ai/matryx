use crate::AppState;
use crate::auth::verify_x_matrix_auth;

use crate::utils::request_helpers::extract_request_uri;
use crate::utils::response_helpers::{build_multipart_media_response, MultipartMediaResponse, MediaContent};
use axum::{
    body::Body,
    extract::{Path, Request, State},
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
    request: Request,
) -> Result<Response<Body>, StatusCode> {
    // Verify X-Matrix authentication using actual request URI (including query parameters)
    let uri = extract_request_uri(&request);
    let auth_result = verify_x_matrix_auth(
        &headers,
        &state.homeserver_name,
        "GET",
        uri,
        None, // No body for GET requests
        state.event_signer.get_signing_engine(),
    )
    .await;
    let _x_matrix_auth = auth_result.map_err(|e| {
        warn!("X-Matrix authentication failed for media download: {}", e);
        StatusCode::from(e)
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
            &_x_matrix_auth.origin,
        )
        .await
        .map_err(|e| {
            debug!("Media not found or access denied: {}", e);
            StatusCode::NOT_FOUND
        })?;

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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(response)
}
