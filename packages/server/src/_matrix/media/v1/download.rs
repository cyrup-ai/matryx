use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

/// GET /_matrix/media/v1/download/{serverName}/{mediaId}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
) -> Result<Response<Body>, StatusCode> {
    // Create MediaService instance with federation support
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo)
        .with_federation_client(
            state.federation_media_client.clone(),
            state.homeserver_name.clone(),
        );

    // Download media using MediaService (v1 API - anonymous access for now)
    let download_result = media_service
        .download_media(&media_id, &server_name, "anonymous")
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Create response body from content
    let body = Body::from(download_result.content);

    // Build response with appropriate headers (v1 compatible)
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(header::CONTENT_LENGTH, download_result.content_length.to_string());

    // Add Content-Disposition header if filename is available
    if let Some(filename) = download_result.filename {
        response = response
            .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{}\"", filename));
    }

    response.body(body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /_matrix/media/v1/download/{serverName}/{mediaId}/{fileName}
pub async fn get_with_filename(
    State(state): State<AppState>,
    Path((server_name, media_id, _file_name)): Path<(String, String, String)>,
) -> Result<Response<Body>, StatusCode> {
    // Same implementation as get() for v1 compatibility
    get(State(state), Path((server_name, media_id))).await
}
