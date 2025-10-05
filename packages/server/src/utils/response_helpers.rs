use crate::error::MatrixError;
use axum::{
    body::Body,
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use uuid::Uuid;

/// Create standardized Matrix JSON response
pub fn matrix_response<T: Serialize>(data: T) -> impl IntoResponse {
    (StatusCode::OK, Json(data))
}

/// Create Matrix error response with proper format
#[allow(dead_code)] // Utility function for Matrix error responses
pub fn matrix_error_response(error: MatrixError) -> impl IntoResponse {
    error.into_response()
}

/// Create JSON response with proper headers and CORS (legacy function)
#[allow(dead_code)] // Legacy utility function for JSON responses
pub fn json_response<T: serde::Serialize>(data: T) -> Result<Json<T>, StatusCode> {
    Ok(Json(data))
}

/// Create media response with security headers
#[allow(dead_code)] // Utility function for media responses with security headers
pub fn media_response(
    content_type: &str,
    content_length: u64,
    filename: Option<&str>,
    body: Body,
) -> Result<Response<Body>, StatusCode> {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .header(header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "X-Requested-With, Content-Type, Authorization");

    if let Some(name) = filename {
        response =
            response.header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{}\"", name));
    }

    response.body(body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Validate Content-Type for inline media (security)
#[allow(dead_code)]
pub fn is_safe_inline_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "text/css"
            | "text/plain"
            | "text/csv"
            | "application/json"
            | "application/ld+json"
            | "image/jpeg"
            | "image/gif"
            | "image/png"
            | "image/apng"
            | "image/webp"
            | "image/avif"
            | "video/mp4"
            | "video/webm"
            | "video/ogg"
            | "video/quicktime"
            | "audio/mp4"
            | "audio/webm"
            | "audio/aac"
            | "audio/mpeg"
            | "audio/ogg"
            | "audio/wave"
            | "audio/wav"
            | "audio/x-wav"
            | "audio/x-pn-wav"
            | "audio/flac"
            | "audio/x-flac"
    )
}

/// Multipart media response containing metadata and content
pub struct MultipartMediaResponse {
    pub metadata: serde_json::Value,
    pub content: MediaContent,
}

/// Media content can be bytes or a redirect location
pub enum MediaContent {
    Bytes {
        data: Vec<u8>,
        content_type: String,
        filename: Option<String>,
    },
    #[allow(dead_code)]
    Redirect { location: String },
}

/// Build Matrix-compliant multipart/mixed response for federation media
pub fn build_multipart_media_response(
    response: MultipartMediaResponse,
) -> Result<Response<Body>, axum::http::Error> {
    let boundary = format!("matrix_media_{}", Uuid::new_v4().simple());

    let mut body_bytes = Vec::new();

    // Part 1: JSON metadata (currently empty object per spec)
    let metadata_part = format!(
        "--{}\r\nContent-Type: application/json\r\n\r\n{}\r\n",
        boundary,
        serde_json::to_string(&response.metadata).unwrap_or_else(|_| "{}".to_string())
    );
    body_bytes.extend_from_slice(metadata_part.as_bytes());

    // Part 2: Media content or redirect
    match response.content {
        MediaContent::Bytes { data, content_type, filename } => {
            let mut part_header = format!("--{}\r\nContent-Type: {}\r\n", boundary, content_type);

            if let Some(name) = filename {
                part_header.push_str(&format!(
                    "Content-Disposition: attachment; filename=\"{}\"\r\n",
                    name
                ));
            }

            part_header.push_str("\r\n");
            body_bytes.extend_from_slice(part_header.as_bytes());
            body_bytes.extend_from_slice(&data); // ✅ PRESERVE BINARY DATA
            body_bytes.extend_from_slice(b"\r\n");
        },
        MediaContent::Redirect { location } => {
            let redirect_part = format!("--{}\r\nLocation: {}\r\n\r\n\r\n", boundary, location);
            body_bytes.extend_from_slice(redirect_part.as_bytes());
        },
    }

    // Final boundary
    let final_boundary = format!("--{}--\r\n", boundary);
    body_bytes.extend_from_slice(final_boundary.as_bytes());

    Response::builder()
        .status(200)
        .header("Content-Type", format!("multipart/mixed; boundary={}", boundary))
        .header("Content-Length", body_bytes.len().to_string())
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .body(Body::from(body_bytes)) // ✅ BINARY-SAFE BODY
}
