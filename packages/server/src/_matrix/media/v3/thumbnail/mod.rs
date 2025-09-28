pub mod by_server_name;

use axum::{
    extract::{Path, Query, State},
    response::Response,
};
use image::{ImageFormat, imageops::FilterType};
use serde::Deserialize;
use std::{io::Cursor, path::PathBuf, time::Duration};
use tracing::{debug, warn};

use crate::{AppState, error::MatrixError};
use crate::utils::response_helpers::{build_multipart_media_response, MultipartMediaResponse, MediaContent};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ThumbnailQuery {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_method")]
    pub method: String,
    pub timeout_ms: Option<u64>,
}

fn default_method() -> String {
    "scale".to_string()
}

#[derive(Debug)]
pub enum ThumbnailMethod {
    Scale,
    Crop,
}

impl ThumbnailMethod {
    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            "scale" => Ok(ThumbnailMethod::Scale),
            "crop" => Ok(ThumbnailMethod::Crop),
            _ => Err("Invalid thumbnail method"),
        }
    }
}

fn is_image_content_type(content_type: &str) -> bool {
    matches!(content_type, "image/jpeg" | "image/png" | "image/gif" | "image/webp")
}

fn get_image_format(content_type: &str) -> Option<ImageFormat> {
    match content_type {
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/png" => Some(ImageFormat::Png),
        "image/gif" => Some(ImageFormat::Gif),
        "image/webp" => Some(ImageFormat::WebP),
        _ => None,
    }
}

pub async fn generate_thumbnail(
    original_path: &PathBuf,
    width: u32,
    height: u32,
    method: ThumbnailMethod,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Load the original image
    let img = image::open(original_path)?;

    // Generate thumbnail based on method
    let thumbnail = match method {
        ThumbnailMethod::Crop => img.resize_to_fill(width, height, FilterType::Lanczos3),
        ThumbnailMethod::Scale => img.resize(width, height, FilterType::Lanczos3),
    };

    // Encode as JPEG for thumbnails
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    thumbnail.write_to(&mut cursor, ImageFormat::Jpeg)?;

    Ok(buffer)
}

pub async fn get_thumbnail(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response, MatrixError> {
    debug!(
        "Client thumbnail request for media_id: {}, server: {}, size: {}x{}",
        media_id, server_name, query.width, query.height
    );

    // Validate media_id format
    if media_id.is_empty() || media_id.len() > 255 {
        warn!("Invalid media_id format: {}", media_id);
        return Err(MatrixError::Unknown);
    }

    // Validate server_name format
    if server_name.is_empty() || server_name.len() > 255 {
        warn!("Invalid server_name format: {}", server_name);
        return Err(MatrixError::Unknown);
    }

    // Validate thumbnail dimensions
    if query.width == 0 || query.height == 0 {
        warn!("Invalid thumbnail dimensions: {}x{}", query.width, query.height);
        return Err(MatrixError::Unknown);
    }

    if query.width > 2048 || query.height > 2048 {
        warn!("Oversized thumbnail dimensions: {}x{}", query.width, query.height);
        return Err(MatrixError::TooLargeFor {
            action: "thumbnail".to_string()
        });
    }

    // Parse thumbnail method
    let method = ThumbnailMethod::from_str(&query.method).map_err(|_| {
        warn!("Invalid thumbnail method: {}", query.method);
        MatrixError::Unknown
    })?;

    // Add timeout validation and application
    let timeout_ms = query.timeout_ms.unwrap_or(20000).min(120000); // Max 2 minutes
    let timeout_duration = Duration::from_millis(timeout_ms);

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Get media metadata to validate content type before thumbnail generation
    let media_repo = MediaRepository::new(state.db.clone());
    let media_metadata = media_repo
        .get_media_info(&media_id, &server_name)
        .await
        .map_err(|_| MatrixError::Unknown)?
        .ok_or(MatrixError::NotFound)?;

    // Validate that the media is an image type before generating thumbnail
    if !is_image_content_type(&media_metadata.content_type) {
        warn!("Attempted to generate thumbnail for non-image content: {}", media_metadata.content_type);
        return Err(MatrixError::Unknown);
    }

    // Validate image format is supported for thumbnail generation
    let _image_format = get_image_format(&media_metadata.content_type)
        .ok_or_else(|| {
            warn!("Unsupported image format for thumbnail: {}", media_metadata.content_type);
            MatrixError::Unknown
        })?;

    debug!("Generating thumbnail for {} image ({}x{})",
           media_metadata.content_type, query.width, query.height);

    // Generate thumbnail using MediaService with validated method
    let method_str = match method {
        ThumbnailMethod::Crop => "crop",
        ThumbnailMethod::Scale => "scale",
    };

    // Apply timeout to MediaService call
    let thumbnail_result = tokio::time::timeout(
        timeout_duration,
        media_service.generate_thumbnail(&media_id, &server_name, query.width, query.height, method_str)
    )
    .await
    .map_err(|_| MatrixError::NotYetUploaded)? // Timeout = content not ready
    .map_err(|e| {
        debug!("Failed to generate thumbnail for {}: {}", media_id, e);
        MatrixError::from(e)
    })?;

    debug!(
        "Serving client thumbnail: media={}, size={}x{}, type={}",
        media_id, thumbnail_result.width, thumbnail_result.height, thumbnail_result.content_type
    );

    // ✅ COMPLIANT: Multipart/mixed response
    let multipart_response = MultipartMediaResponse {
        metadata: serde_json::json!({}), // Empty object per spec
        content: MediaContent::Bytes {
            data: thumbnail_result.thumbnail,
            content_type: thumbnail_result.content_type,
            filename: None,
        },
    };

    let response = build_multipart_media_response(multipart_response)
        .map_err(|e| {
            debug!("Failed to build multipart thumbnail response: {}", e);
            MatrixError::Unknown
        })?;

    debug!("Successfully serving thumbnail: {}", media_id);
    Ok(response)
}
