use axum::{
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    body::Body,
};
use serde::Deserialize;
use tracing::{debug, error, warn};
use tokio::fs::{File, create_dir_all};
use tokio_util::io::ReaderStream;
use image::{ImageFormat, imageops::FilterType};

use crate::state::AppState;
use crate::auth::verify_x_matrix_auth;
use crate::error::MatrixError;
use crate::utils::request_helpers::extract_request_uri;
use crate::utils::response_helpers::{build_multipart_media_response, MultipartMediaResponse, MediaContent};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    room::RoomRepository,
    membership::MembershipRepository,
};
use std::sync::Arc;
use std::time::Duration;

/// Query parameters for thumbnail download
#[derive(Debug, Deserialize)]
pub struct ThumbnailQuery {
    width: u32,
    height: u32,
    method: Option<String>,
    animated: Option<bool>,
    timeout_ms: Option<u64>,
}

/// GET /_matrix/federation/v1/media/thumbnail/{mediaId}
///
/// Downloads thumbnail content from the local server for federation.
/// This endpoint requires X-Matrix authentication and serves thumbnails
/// that were previously generated for uploaded media.
pub async fn get(
    State(state): State<AppState>,
    Path(media_id): Path<String>,
    Query(query): Query<ThumbnailQuery>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, MatrixError> {
    // Verify X-Matrix authentication using actual request URI (including all query parameters)
    let uri = extract_request_uri(&request);
    let auth_result = verify_x_matrix_auth(
        &headers,
        &state.homeserver_name,
        "GET",
        uri,
        None, // No body for GET requests
        state.event_signer.get_signing_engine(),
    ).await;
    let _x_matrix_auth = auth_result.map_err(|e| {
        warn!("X-Matrix authentication failed for thumbnail download: {}", e);
        MatrixError::from(e)
    })?;

    debug!(
        "Federation thumbnail download request for media_id: {}, size: {}x{}", 
        media_id, query.width, query.height
    );

    // Validate media_id format
    if media_id.is_empty() || media_id.len() > 255 {
        warn!("Invalid media_id format: {}", media_id);
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

    let method = query.method.as_deref().unwrap_or("scale");
    if !matches!(method, "crop" | "scale") {
        warn!("Invalid thumbnail method: {}", method);
        return Err(MatrixError::Unknown);
    }

    // Add timeout validation and application
    let timeout_ms = query.timeout_ms.unwrap_or(20000).min(120000); // Max 2 minutes
    let timeout_duration = Duration::from_millis(timeout_ms);

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Apply timeout to MediaService call
    let thumbnail_result = tokio::time::timeout(
        timeout_duration,
        media_service.generate_thumbnail(
            &media_id,
            &state.homeserver_name,
            query.width,
            query.height,
            method,
        )
    )
    .await
    .map_err(|_| MatrixError::NotYetUploaded)? // Timeout = content not ready
    .map_err(|e| {
        debug!("Failed to generate thumbnail for {}: {}", media_id, e);
        MatrixError::from(e)
    })?;

    debug!(
        "Serving federation thumbnail: media={}, size={}x{}, type={}",
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
            error!("Failed to build multipart thumbnail response: {}", e);
            MatrixError::Unknown
        })?;

    Ok(response)
}

/// Check if content type supports thumbnail generation
fn is_thumbnailable_content_type(content_type: &str) -> bool {
    matches!(content_type, 
        "image/jpeg" | "image/jpg" | "image/png" | "image/gif" | 
        "image/webp" | "image/bmp" | "image/tiff" | "image/svg+xml"
    )
}

/// Serve thumbnail file from disk
async fn serve_thumbnail_file(
    file_path: &str,
    content_type: &str,
    content_length: u64,
) -> Result<Response, StatusCode> {
    let file = File::open(file_path).await.map_err(|e| {
        error!("Failed to open thumbnail file {}: {}", file_path, e);
        match e.kind() {
            std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Content-Length", content_length.to_string())
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .header("Content-Security-Policy", 
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .header("Access-Control-Allow-Headers", "Authorization, Content-Type")
        .body(body)
        .map_err(|e| {
            error!("Failed to build thumbnail response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}

/// Generate thumbnail for media with image processing
async fn generate_thumbnail(
    state: &AppState,
    media_id: &str,
    original_path: &str,
    content_type: &str,
    width: u32,
    height: u32,
    method: &str,
    animated: bool,
) -> Result<(String, String, u64), Box<dyn std::error::Error + Send + Sync>> {
    // SUBTASK6: Validate thumbnail size limits
    const MAX_THUMBNAIL_SIZE: u32 = 2048;
    const MIN_THUMBNAIL_SIZE: u32 = 1;
    
    if width < MIN_THUMBNAIL_SIZE || height < MIN_THUMBNAIL_SIZE {
        return Err(format!("Thumbnail size {}x{} too small (minimum {}x{})", 
                          width, height, MIN_THUMBNAIL_SIZE, MIN_THUMBNAIL_SIZE).into());
    }
    
    if width > MAX_THUMBNAIL_SIZE || height > MAX_THUMBNAIL_SIZE {
        warn!("Oversized thumbnail request: {}x{} for media {}", width, height, media_id);
        return Err(format!("Thumbnail size {}x{} exceeds maximum {}x{}", 
                          width, height, MAX_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE).into());
    }

    // SUBTASK2: Load source image and implement core thumbnail generation
    let img = image::open(original_path)
        .map_err(|e| format!("Failed to load image {}: {}", original_path, e))?;

    // Resize based on method with high-quality filter
    let thumbnail = match method {
        "crop" => img.resize_to_fill(width, height, FilterType::Lanczos3),
        "scale" => img.resize(width, height, FilterType::Lanczos3),
        _ => return Err(format!("Invalid scaling method: {}", method).into()),
    };

    // SUBTASK3: Generate thumbnail file path and create directory structure
    let thumbnail_dir = format!("media/thumbnails/{}", &media_id[0..2]);
    create_dir_all(&thumbnail_dir).await
        .map_err(|e| format!("Failed to create thumbnail directory {}: {}", thumbnail_dir, e))?;
    
    let thumbnail_filename = format!("{}_{}_{}x{}_{}.png", 
                                   media_id, method, width, height, 
                                   if animated { "animated" } else { "static" });
    let thumbnail_path = format!("{}/{}", thumbnail_dir, thumbnail_filename);

    // SUBTASK4: Implement format detection and conversion
    let output_content_type = match content_type {
        "image/png" | "image/gif" => "image/png", // Preserve transparency
        _ => "image/jpeg", // Optimize for photos
    };

    // Save thumbnail with appropriate format
    if output_content_type == "image/png" {
        thumbnail.save_with_format(&thumbnail_path, ImageFormat::Png)
            .map_err(|e| format!("Failed to save PNG thumbnail {}: {}", thumbnail_path, e))?;
    } else {
        thumbnail.save_with_format(&thumbnail_path, ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to save JPEG thumbnail {}: {}", thumbnail_path, e))?;
    }

    // Get file metadata
    let metadata = tokio::fs::metadata(&thumbnail_path).await
        .map_err(|e| format!("Failed to get thumbnail metadata {}: {}", thumbnail_path, e))?;
    let file_size = metadata.len();

    // SUBTASK5: Store thumbnail record in database
    let insert_query = "
        INSERT INTO thumbnails (
            thumbnail_id, media_id, width, height, method, animated,
            file_path, content_type, content_length, created_at
        ) VALUES (
            $thumbnail_id, $media_id, $width, $height, $method, $animated,
            $file_path, $content_type, $content_length, time::now()
        )
    ";
    
    let thumbnail_id = format!("{}_{}_{}x{}_{}", media_id, method, width, height, animated);
    
    let media_repo = MediaRepository::new(state.db.clone());
    media_repo
        .store_media_thumbnail(
            media_id,
            "localhost", // In real implementation, would get from server context
            width,
            height,
            method,
            &thumbnail_data,
        )
        .await
        .map_err(|e| format!("Failed to store thumbnail: {}", e))?;

    debug!("Generated thumbnail: {} ({}x{}, {}, {} bytes)", 
           thumbnail_path, width, height, method, file_size);

    Ok((thumbnail_path, output_content_type.to_string(), file_size))
}