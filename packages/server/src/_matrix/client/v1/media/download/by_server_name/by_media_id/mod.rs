use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};

use crate::auth::authenticated_user::AuthenticatedUser;
use crate::utils::response_helpers::calculate_content_disposition;
use crate::AppState;
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

/// GET /_matrix/client/v1/media/download/{serverName}/{mediaId}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    // Apply timeout validation (default: 20s, max: 120s)
    let timeout_ms = query.timeout_ms.unwrap_or(20000).min(120000);
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

    // Download media with timeout and authentication
    let download_result = tokio::time::timeout(
        timeout_duration,
        media_service.download_media(&media_id, &server_name, &user.user_id),
    )
    .await
    .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Generate Content-Disposition header
    let disposition = calculate_content_disposition(
        &download_result.content_type,
        download_result.filename.as_deref(),
    );

    // Create response body from content
    let body = Body::from(download_result.content);

    // Build response with security headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(
            header::CONTENT_LENGTH,
            download_result.content_length.to_string(),
        )
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(
            "Content-Security-Policy",
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';",
        )
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
