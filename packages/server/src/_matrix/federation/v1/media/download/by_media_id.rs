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

/// Query parameters for media download
#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    timeout_ms: Option<u64>,
}

/// GET /_matrix/federation/v1/media/download/{mediaId}
///
/// Downloads media content from the local server for federation.
/// This endpoint requires X-Matrix authentication and serves media
/// that was previously uploaded to this homeserver.
pub async fn get(
    State(state): State<AppState>,
    Path(media_id): Path<String>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    // Verify X-Matrix authentication
    let auth_result = verify_x_matrix_auth(&headers, &state.server_name, &state.signing_key).await;
    let _x_matrix_auth = auth_result.map_err(|e| {
        warn!("X-Matrix authentication failed for media download: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    debug!("Federation media download request for media_id: {}", media_id);

    // Validate media_id format
    if media_id.is_empty() || media_id.len() > 255 {
        warn!("Invalid media_id format: {}", media_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Query media metadata from database
    let media_query = "
        SELECT 
            media_id, 
            content_type, 
            content_length, 
            file_path, 
            upload_name, 
            created_at,
            user_id
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

    // Extract media metadata
    let content_type = media_record
        .get("content_type")
        .and_then(|v| v.as_str())
        .unwrap_or("application/octet-stream");

    let content_length = media_record
        .get("content_length")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let file_path = media_record
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            error!("Missing file_path for media: {}", media_id);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let upload_name = media_record
        .get("upload_name")
        .and_then(|v| v.as_str());

    debug!(
        "Serving media: id={}, type={}, size={}, path={}",
        media_id, content_type, content_length, file_path
    );

    // Check if file exists on disk
    let file = File::open(file_path).await.map_err(|e| {
        error!("Failed to open media file {}: {}", file_path, e);
        match e.kind() {
            std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    })?;

    // Create file stream for response
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // Build response with appropriate headers
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Content-Length", content_length.to_string())
        .header("Cache-Control", "public, max-age=31536000, immutable"); // 1 year cache

    // Add Content-Disposition header if upload name is available
    if let Some(name) = upload_name {
        // Sanitize filename for header
        let sanitized_name = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(*c, '.' | '-' | '_' | ' '))
            .collect::<String>();
        
        if !sanitized_name.is_empty() {
            response = response.header(
                "Content-Disposition",
                format!("inline; filename=\"{}\"", sanitized_name)
            );
        }
    }

    // Add CORS headers for federation
    response = response
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .header("Access-Control-Allow-Headers", "Authorization, Content-Type");

    let response = response.body(body).map_err(|e| {
        error!("Failed to build media response: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    debug!("Successfully serving media: {}", media_id);
    Ok(response)
}