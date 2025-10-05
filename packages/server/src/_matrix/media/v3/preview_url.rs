use crate::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};
use serde::{Deserialize, Serialize};

use std::sync::Arc;

#[derive(Deserialize)]
pub struct PreviewQuery {
    pub url: String,
    #[serde(default)]
    #[allow(dead_code)]
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
    // Production URL preview implementation

    // 1. Fetch the URL content with timeout and size limits
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("Matrix Homeserver {}", homeserver_name))
        .build()?;

    let response = client.get(url).send().await?;

    // Validate content type - only process HTML content
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("text/html") {
        return Err("URL does not serve HTML content".into());
    }

    // Limit response size to prevent abuse (1MB max)
    let body = response.bytes().await?;
    if body.len() > 1024 * 1024 {
        return Err("Response too large".into());
    }

    let html = String::from_utf8_lossy(&body);

    // 2. Parse HTML for OpenGraph/meta tags
    let mut title = None;
    let mut description = None;
    let mut image_url = None;

    // Simple regex-based parsing for OpenGraph tags
    if let Ok(title_regex) =
        regex::Regex::new(r#"<meta[^>]+property="og:title"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = title_regex.captures(&html)
    {
        title = Some(cap[1].to_string());
    }

    if let Ok(desc_regex) =
        regex::Regex::new(r#"<meta[^>]+property="og:description"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = desc_regex.captures(&html)
    {
        description = Some(cap[1].to_string());
    }

    if let Ok(img_regex) =
        regex::Regex::new(r#"<meta[^>]+property="og:image"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = img_regex.captures(&html)
    {
        image_url = Some(cap[1].to_string());
    }

    // Fallback to HTML title tag if no OpenGraph title
    if title.is_none()
        && let Ok(html_title_regex) = regex::Regex::new(r"<title>([^<]*)</title>")
        && let Some(cap) = html_title_regex.captures(&html)
    {
        title = Some(cap[1].trim().to_string());
    }

    // Fallback to meta description if no OpenGraph description
    if description.is_none()
        && let Ok(meta_desc_regex) =
            regex::Regex::new(r#"<meta[^>]+name="description"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = meta_desc_regex.captures(&html)
    {
        description = Some(cap[1].to_string());
    }

    // 3. Download and store preview image if available
    let mut image_uri = None;
    let mut image_size = None;

    if let Some(img_url) = image_url {
        // Resolve relative URLs
        let absolute_img_url = if img_url.starts_with("http://") || img_url.starts_with("https://")
        {
            img_url
        } else if img_url.starts_with("//") {
            format!("https:{}", img_url)
        } else if img_url.starts_with('/') {
            // Absolute path - extract base URL
            if let Ok(parsed_url) = url::Url::parse(url) {
                format!(
                    "{}://{}{}",
                    parsed_url.scheme(),
                    parsed_url.host_str().unwrap_or(""),
                    img_url
                )
            } else {
                img_url // Fallback
            }
        } else {
            // Relative path
            if let Ok(parsed_url) = url::Url::parse(url) {
                if let Ok(joined) = parsed_url.join(&img_url) {
                    joined.to_string()
                } else {
                    img_url
                }
            } else {
                img_url
            }
        };

        // Download preview image (with size limits)
        match client.get(&absolute_img_url).send().await {
            Ok(img_response) => {
                let img_content_type = img_response
                    .headers()
                    .get("content-type")
                    .and_then(|ct| ct.to_str().ok())
                    .unwrap_or("image/png")
                    .to_string();

                // Only download images up to 5MB
                match img_response.bytes().await {
                    Ok(img_data) if img_data.len() <= 5 * 1024 * 1024 => {
                        // Store image using MediaService
                        if let Ok(upload_result) = media_service
                            .upload_media(
                                &format!("@system:{}", homeserver_name),
                                &img_data,
                                &img_content_type,
                                Some("preview_image"),
                            )
                            .await
                        {
                            image_uri = Some(upload_result.content_uri);
                            image_size = Some(img_data.len() as u64);
                        }
                    },
                    _ => {
                        // Image too large or download failed, skip
                    },
                }
            },
            Err(_) => {
                // Image download failed, skip
            },
        }
    }

    Ok(PreviewResponse { title, description, image: image_uri, image_size })
}
