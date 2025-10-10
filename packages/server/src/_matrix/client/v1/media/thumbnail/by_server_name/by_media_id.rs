use crate::AppState;
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::utils::response_helpers::media_response;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

// Use shared ThumbnailQuery from v3
use crate::_matrix::media::v3::thumbnail::ThumbnailQuery;

/// GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}
///
/// Authenticated endpoint that returns binary thumbnail image data.
/// Requires Bearer token in Authorization header.
pub async fn get(
    user: AuthenticatedUser,  // Auto-validates Bearer token
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response<Body>, StatusCode> {
    // Validate thumbnail dimensions
    if query.width == 0 || query.height == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Matrix spec guidance: max 2048x2048 to prevent abuse
    if query.width > 2048 || query.height > 2048 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate method parameter
    let method = query.method.as_str();
    if !matches!(method, "crop" | "scale") {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Validate user has access to this media
    let can_access = media_service
        .validate_media_access(&media_id, &server_name, &user.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_access {
        return Err(StatusCode::FORBIDDEN);
    }

    // Generate thumbnail using MediaService
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, method)
        .await
        .map_err(|e| {
            use matryx_surrealdb::repository::media_service::MediaError;
            match e {
                MediaError::NotFound => StatusCode::NOT_FOUND,
                MediaError::NotYetUploaded => StatusCode::GATEWAY_TIMEOUT,
                MediaError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
                MediaError::UnsupportedFormat => StatusCode::BAD_REQUEST,
                MediaError::AccessDenied(_) => StatusCode::FORBIDDEN,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    // Determine filename from content type
    let filename = match thumbnail_result.content_type.as_str() {
        "image/png" => "thumbnail.png",
        "image/jpeg" => "thumbnail.jpg",
        "image/gif" => "thumbnail.gif",
        "image/webp" => "thumbnail.webp",
        _ => "thumbnail.png",
    };

    // Return binary image data with proper headers
    media_response(
        &thumbnail_result.content_type,
        thumbnail_result.thumbnail.len() as u64,
        Some(filename),
        Body::from(thumbnail_result.thumbnail),
    )
}
