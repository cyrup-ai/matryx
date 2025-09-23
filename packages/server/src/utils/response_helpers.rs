use crate::error::MatrixError;
use axum::{
    body::Body,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use serde_json::json;

/// Create standardized Matrix JSON response
pub fn matrix_response<T: Serialize>(data: T) -> impl IntoResponse {
    (StatusCode::OK, Json(data))
}

/// Create Matrix error response with proper format
pub fn matrix_error_response(error: MatrixError) -> impl IntoResponse {
    error.into_response()
}

/// Create JSON response with proper headers and CORS (legacy function)
pub fn json_response<T: serde::Serialize>(data: T) -> Result<Json<T>, StatusCode> {
    Ok(Json(data))
}

/// Create media response with security headers
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
pub fn is_safe_inline_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "text/css" |
            "text/plain" |
            "text/csv" |
            "application/json" |
            "application/ld+json" |
            "image/jpeg" |
            "image/gif" |
            "image/png" |
            "image/apng" |
            "image/webp" |
            "image/avif" |
            "video/mp4" |
            "video/webm" |
            "video/ogg" |
            "video/quicktime" |
            "audio/mp4" |
            "audio/webm" |
            "audio/aac" |
            "audio/mpeg" |
            "audio/ogg" |
            "audio/wave" |
            "audio/wav" |
            "audio/x-wav" |
            "audio/x-pn-wav" |
            "audio/flac" |
            "audio/x-flac"
    )
}
