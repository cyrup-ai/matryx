use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};
use tracing::warn;

use crate::auth::authenticated_user::AuthenticatedUser;
use crate::utils::response_helpers::calculate_content_disposition;
use crate::AppState;
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

/// GET /_matrix/media/v3/download/{serverName}/{mediaId}/{fileName}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id, _file_name)): Path<(String, String, String)>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    warn!(
        endpoint = "GET /_matrix/media/v3/download/{serverName}/{mediaId}/{fileName}",
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

    // Download media using MediaService with authenticated user
    let download_result = media_service
        .download_media(&media_id, &server_name, &user.user_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Create response body from content
    let body = Body::from(download_result.content);

    // Calculate Content-Disposition header based on content type
    let content_disposition = calculate_content_disposition(
        &download_result.content_type,
        download_result.filename.as_deref(),
    );

    // Build response with appropriate headers and security headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(header::CONTENT_LENGTH, download_result.content_length.to_string())
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header(header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "X-Requested-With, Content-Type, Authorization")
        .header("Deprecation", "true")
        .header("Sunset", "Wed, 01 Sep 2024 00:00:00 GMT")
        .header("Link", r#"<https://spec.matrix.org/v1.11/client-server-api/#content-repository>; rel="deprecation""#)
        .header("X-Matrix-Deprecated-Endpoint", "Use /_matrix/client/v1/media/* instead");

    response.body(body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
