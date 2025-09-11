use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    body::Body,
};
use serde::Deserialize;
use tracing::{debug, error, warn};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::state::AppState;
use crate::auth::verify_x_matrix_auth;

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
) -> Result<Response, StatusCode> {
    // Verify X-Matrix authentication
    let auth_result = verify_x_matrix_auth(&headers, &state.server_name, &state.signing_key).await;
    let _x_matrix_auth = auth_result.map_err(|e| {
        warn!("X-Matrix authentication failed for thumbnail download: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    debug!(
        "Federation thumbnail download request for media_id: {}, size: {}x{}", 
        media_id, query.width, query.height
    );

    // Validate media_id format
    if media_id.is_empty() || media_id.len() > 255 {
        warn!("Invalid media_id format: {}", media_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate thumbnail dimensions
    if query.width == 0 || query.height == 0 || query.width > 2048 || query.height > 2048 {
        warn!("Invalid thumbnail dimensions: {}x{}", query.width, query.height);
        return Err(StatusCode::BAD_REQUEST);
    }

    let method = query.method.as_deref().unwrap_or("scale");
    if !matches!(method, "crop" | "scale") {
        warn!("Invalid thumbnail method: {}", method);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Query original media metadata
    let media_query = "
        SELECT 
            media_id, 
            content_type, 
            file_path, 
            upload_name,
            created_at
        FROM media 
        WHERE media_id = $media_id
        LIMIT 1
    ";

    let mut media_result = state
        .db
        .query(media_query)
        .bind(("media_id", media_id.clone()))
        .await
        .map_err(|e| {
            error!("Database query failed for media lookup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let media_records: Vec<serde_json::Value> = media_result.take(0).map_err(|e| {
        error!("Failed to parse media query result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let media_record = media_records.first().ok_or_else(|| {
        debug!("Media not found: {}", media_id);
        StatusCode::NOT_FOUND
    })?;

    let original_content_type = media_record
        .get("content_type")
        .and_then(|v| v.as_str())
        .unwrap_or("application/octet-stream");

    // Check if media type supports thumbnailing
    if !is_thumbnailable_content_type(original_content_type) {
        debug!("Media type {} not thumbnailable: {}", original_content_type, media_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Look for existing thumbnail
    let thumbnail_query = "
        SELECT 
            thumbnail_id,
            content_type,
            content_length,
            file_path,
            width,
            height,
            method,
            animated
        FROM thumbnails 
        WHERE media_id = $media_id 
        AND width = $width 
        AND height = $height 
        AND method = $method
        AND animated = $animated
        LIMIT 1
    ";

    let animated = query.animated.unwrap_or(false);
    let mut thumbnail_result = state
        .db
        .query(thumbnail_query)
        .bind(("media_id", media_id.clone()))
        .bind(("width", query.width as i64))
        .bind(("height", query.height as i64))
        .bind(("method", method.to_string()))
        .bind(("animated", animated))
        .await
        .map_err(|e| {
            error!("Database query failed for thumbnail lookup: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let thumbnail_records: Vec<serde_json::Value> = thumbnail_result.take(0).map_err(|e| {
        error!("Failed to parse thumbnail query result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(thumbnail_record) = thumbnail_records.first() {
        // Serve existing thumbnail
        let content_type = thumbnail_record
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("image/jpeg");

        let content_length = thumbnail_record
            .get("content_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let file_path = thumbnail_record
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                error!("Missing file_path for thumbnail: {}", media_id);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        debug!(
            "Serving existing thumbnail: media={}, size={}x{}, path={}",
            media_id, query.width, query.height, file_path
        );

        return serve_thumbnail_file(file_path, content_type, content_length).await;
    }

    // Generate thumbnail on-demand if not found
    debug!(
        "Generating thumbnail on-demand: media={}, size={}x{}, method={}",
        media_id, query.width, query.height, method
    );

    let original_file_path = media_record
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            error!("Missing file_path for original media: {}", media_id);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Generate and serve thumbnail
    match generate_thumbnail(
        &state,
        &media_id,
        original_file_path,
        original_content_type,
        query.width,
        query.height,
        method,
        animated,
    ).await {
        Ok((thumbnail_path, thumbnail_content_type, thumbnail_size)) => {
            serve_thumbnail_file(&thumbnail_path, &thumbnail_content_type, thumbnail_size).await
        },
        Err(e) => {
            error!("Failed to generate thumbnail for {}: {}", media_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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

/// Generate thumbnail for media (placeholder implementation)
async fn generate_thumbnail(
    _state: &AppState,
    _media_id: &str,
    _original_path: &str,
    _content_type: &str,
    _width: u32,
    _height: u32,
    _method: &str,
    _animated: bool,
) -> Result<(String, String, u64), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: Implement actual thumbnail generation using image processing library
    // For now, return an error to indicate thumbnails need to be pre-generated
    Err("Thumbnail generation not yet implemented - thumbnails must be pre-generated".into())
}