use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use serde_json::Value;
use std::collections::HashMap;
use tokio::fs;
use tokio_util::io::ReaderStream;

use crate::AppState;
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

/// GET /_matrix/media/v3/download/{serverName}/{mediaId}/{fileName}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id, _file_name)): Path<(String, String, String)>,
) -> Result<Response<Body>, StatusCode> {
    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Download media using MediaService (anonymous access for now)
    let download_result = media_service
        .download_media(&media_id, &server_name, "anonymous")
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Create response body from content
    let body = Body::from(download_result.content);

    // Build response with appropriate headers and security headers
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(header::CONTENT_LENGTH, download_result.content_length.to_string())
        .header(header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "X-Requested-With, Content-Type, Authorization");

    // Add Content-Disposition header if filename is available
    if let Some(filename) = download_result.filename {
        response = response
            .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{}\"", filename));
    }

    response.body(body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
