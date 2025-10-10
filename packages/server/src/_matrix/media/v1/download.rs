use crate::auth::authenticated_user::AuthenticatedUser;
use crate::utils::response_helpers::calculate_content_disposition;
use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

/// GET /_matrix/media/v1/download/{serverName}/{mediaId}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    user: AuthenticatedUser,
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

    // Download media using MediaService with authenticated user
    let download_result = media_service
        .download_media(&media_id, &server_name, &user.user_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Create response body from content
    let body = Body::from(download_result.content);

    // Build response with appropriate headers (v1 compatible)
    let content_disposition = calculate_content_disposition(
        &download_result.content_type,
        download_result.filename.as_deref(),
    );

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(header::CONTENT_LENGTH, download_result.content_length.to_string())
        .header(header::CONTENT_DISPOSITION, content_disposition);

    response.body(body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /_matrix/media/v1/download/{serverName}/{mediaId}/{fileName}
pub async fn get_with_filename(
    State(state): State<AppState>,
    Path((server_name, media_id, _file_name)): Path<(String, String, String)>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    // Same implementation as get() for v1 compatibility
    get(State(state), Path((server_name, media_id)), user).await
}
