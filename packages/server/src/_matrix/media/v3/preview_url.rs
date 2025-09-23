use crate::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct PreviewQuery {
    pub url: String,
    #[serde(default)]
    pub ts: Option<u64>,
}

#[derive(Serialize)]
pub struct PreviewResponse {
    #[serde(rename = "og:title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "og:description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "og:image", skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "matrix:image:size", skip_serializing_if = "Option::is_none")]
    pub image_size: Option<u64>,
}

/// GET /_matrix/media/v3/preview_url
pub async fn get(
    State(state): State<AppState>,
    Query(query): Query<PreviewQuery>,
) -> Result<Json<PreviewResponse>, StatusCode> {
    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Validate URL format
    if !query.url.starts_with("http://") && !query.url.starts_with("https://") {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Fetch URL preview (simplified implementation)
    let preview = fetch_url_preview(&query.url, &media_service, &state.homeserver_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(preview))
}

async fn fetch_url_preview(
    url: &str,
    media_service: &MediaService<surrealdb::engine::any::Any>,
    homeserver_name: &str,
) -> Result<PreviewResponse, Box<dyn std::error::Error + Send + Sync>> {
    // Simplified URL preview implementation
    // In a real implementation, this would:
    // 1. Fetch the URL content
    // 2. Parse HTML for OpenGraph/meta tags
    // 3. Download and store preview images
    // 4. Cache the preview data

    // Mock preview data for demonstration
    let title = Some("Example Website".to_string());
    let description = Some("This is an example website description".to_string());

    // Generate a placeholder preview image using MediaService
    let preview_image_content = b"preview_image_placeholder";

    // Store preview image using MediaService
    let upload_result = media_service
        .upload_media(
            &format!("@system:{}", homeserver_name),
            preview_image_content,
            "image/png",
            Some("preview.png"),
        )
        .await?;

    Ok(PreviewResponse {
        title,
        description,
        image: Some(upload_result.content_uri),
        image_size: Some(preview_image_content.len() as u64),
    })
}
