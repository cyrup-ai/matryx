use crate::auth::authenticated_user::AuthenticatedUser;
use crate::AppState;
use crate::utils::response_helpers::media_response;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
};
use tracing::warn;
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};

use std::sync::Arc;

// Use shared ThumbnailQuery from parent module
use super::super::ThumbnailQuery;

/// GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}
pub async fn get(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response<Body>, StatusCode> {
    warn!(
        endpoint = "GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}",
        "Deprecated endpoint accessed - clients should migrate to /_matrix/client/v1/media/*"
    );

    // Check freeze status
    if state.config.media_config.freeze_enabled {
        let media_repo = Arc::new(MediaRepository::new(state.db.clone()));

        if let Ok(Some(media_info)) = media_repo.get_media_info(&media_id, &server_name).await {
            let is_idp_icon = media_info.is_idp_icon.unwrap_or(false);

            if !is_idp_icon && state.config.media_config.is_frozen(media_info.created_at) {
                warn!(
                    media_id = media_id,
                    uploaded_at = %media_info.created_at,
                    "Blocking post-freeze media on deprecated endpoint"
                );

                return Err(StatusCode::NOT_FOUND);
            }
        }
    }

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Generate thumbnail using MediaService
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, &query.method)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Return binary thumbnail data per Matrix spec
    let mut response = media_response(
        &thumbnail_result.content_type,
        thumbnail_result.thumbnail.len() as u64,
        Some("thumbnail.png"),
        Body::from(thumbnail_result.thumbnail)
    )?;

    // Add deprecation headers
    let headers = response.headers_mut();
    headers.insert("Deprecation", "true".parse().unwrap());
    headers.insert("Sunset", "Wed, 01 Sep 2024 00:00:00 GMT".parse().unwrap());
    headers.insert("Link", r#"<https://spec.matrix.org/v1.11/client-server-api/#content-repository>; rel="deprecation""#.parse().unwrap());
    headers.insert("X-Matrix-Deprecated-Endpoint", "Use /_matrix/client/v1/media/* instead".parse().unwrap());

    Ok(response)
}
