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

use std::net::IpAddr;
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
    #[serde(rename = "og:image:type", skip_serializing_if = "Option::is_none")]
    pub image_type: Option<String>,
    #[serde(rename = "matrix:image:size", skip_serializing_if = "Option::is_none")]
    pub image_size: Option<u64>,
}

/// Validate URL is not targeting internal networks (SSRF protection)
async fn validate_url_safety(url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parsed = url::Url::parse(url)?;
    
    // Only allow http/https
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Invalid scheme".into());
    }
    
    // Get host
    let host = parsed.host_str().ok_or("No host")?;
    
    // Resolve hostname to IP addresses
    let addrs: Vec<IpAddr> = tokio::net::lookup_host(format!("{}:80", host))
        .await?
        .map(|addr| addr.ip())
        .collect();
    
    // Check each resolved IP
    for addr in addrs {
        if is_private_ip(&addr) {
            return Err("Access to private IPs is forbidden".into());
        }
    }
    
    Ok(())
}

/// Check if an IP address is private/internal
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_private() ||
            ipv4.is_loopback() ||
            ipv4.is_link_local() ||
            ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254 || // AWS metadata
            ipv4.octets()[0] == 0 // 0.0.0.0/8
        },
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback() ||
            ipv6.is_unicast_link_local() ||
            ipv6.is_unique_local() ||
            ipv6.is_unspecified()
        }
    }
}

/// GET /_matrix/client/v1/media/preview_url
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

    // Fetch URL preview
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
    // 1. Fetch the URL content with timeout and size limits
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("Matrix Homeserver {}", homeserver_name))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    // Manual redirect handling with SSRF validation at each step
    let mut current_url = url.to_string();
    let mut redirect_count = 0;

    let response = loop {
        // Validate URL is safe before fetching
        validate_url_safety(&current_url).await?;
        
        let resp = client.get(&current_url).send().await?;
        
        if resp.status().is_redirection() {
            if redirect_count >= 3 {
                return Err("Too many redirects".into());
            }
            
            if let Some(location) = resp.headers().get("location") {
                current_url = location.to_str()?.to_string();
                redirect_count += 1;
                continue;
            }
            return Err("Redirect without location".into());
        }
        
        // Not a redirect, use this response
        break resp;
    };

    // 2. Validate content type - only process HTML content
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("text/html") {
        return Err("URL does not serve HTML content".into());
    }

    // 3. Limit response size to prevent abuse (1MB max)
    let body = response.bytes().await?;
    if body.len() > 1024 * 1024 {
        return Err("Response too large".into());
    }

    let html = String::from_utf8_lossy(&body);

    // 4. Parse HTML for OpenGraph/meta tags
    let mut title = None;
    let mut description = None;
    let mut image_url = None;

    // Extract og:title
    if let Ok(title_regex) =
        regex::Regex::new(r#"(?i)<meta[^>]+property="og:title"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = title_regex.captures(&html)
    {
        title = Some(cap[1].to_string());
    }

    // Extract og:description
    if let Ok(desc_regex) =
        regex::Regex::new(r#"(?i)<meta[^>]+property="og:description"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = desc_regex.captures(&html)
    {
        description = Some(cap[1].to_string());
    }

    // Extract og:image
    if let Ok(img_regex) =
        regex::Regex::new(r#"(?i)<meta[^>]+property="og:image"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = img_regex.captures(&html)
    {
        image_url = Some(cap[1].to_string());
    }

    // 5. Fallback to HTML title tag if no OpenGraph title
    if title.is_none()
        && let Ok(html_title_regex) = regex::Regex::new(r"(?i)<title>([^<]*)</title>")
        && let Some(cap) = html_title_regex.captures(&html)
    {
        title = Some(cap[1].trim().to_string());
    }

    // 6. Fallback to meta description if no OpenGraph description
    if description.is_none()
        && let Ok(meta_desc_regex) =
            regex::Regex::new(r#"(?i)<meta[^>]+name="description"[^>]+content="([^"]*)"[^>]*>"#)
        && let Some(cap) = meta_desc_regex.captures(&html)
    {
        description = Some(cap[1].to_string());
    }

    // 7. Download and store preview image if available
    let mut image_uri = None;
    let mut image_size = None;
    let mut image_type = None;

    if let Some(img_url) = image_url {
        // Resolve relative URLs to absolute
        let absolute_img_url = resolve_url(url, &img_url)?;

        // Download preview image (with size limits)
        match client.get(&absolute_img_url).send().await {
            Ok(img_response) => {
                let img_content_type = img_response
                    .headers()
                    .get("content-type")
                    .and_then(|ct| ct.to_str().ok())
                    .unwrap_or("image/png")
                    .to_string();

                // Validate content type is actually an image
                if !img_content_type.starts_with("image/") {
                    // Skip non-image content
                } else {
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
                                image_type = Some(img_content_type);
                            }
                        },
                        _ => {
                            // Image too large or download failed, skip
                        },
                    }
                }
            },
            Err(_) => {
                // Image download failed, skip
            },
        }
    }

    Ok(PreviewResponse { title, description, image: image_uri, image_type, image_size })
}

/// Resolve relative URLs to absolute URLs
fn resolve_url(base_url: &str, relative_url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if relative_url.starts_with("http://") || relative_url.starts_with("https://") {
        Ok(relative_url.to_string())
    } else if relative_url.starts_with("//") {
        Ok(format!("https:{}", relative_url))
    } else if let Ok(base) = url::Url::parse(base_url) {
        if let Ok(joined) = base.join(relative_url) {
            Ok(joined.to_string())
        } else {
            Ok(relative_url.to_string())
        }
    } else {
        Ok(relative_url.to_string())
    }
}
