use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    body::Body,
};
use serde_json::Value;
use std::collections::HashMap;
use tokio::fs;
use tokio_util::io::ReaderStream;

use crate::{
    database::SurrealRepository,
    AppState,
};

pub async fn download_media(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
) -> Result<Response<Body>, StatusCode> {
    // Query media metadata from database
    let query = "SELECT * FROM media_files WHERE server_name = $server_name AND media_id = $media_id";
    let mut params = HashMap::new();
    params.insert("server_name".to_string(), Value::String(server_name));
    params.insert("media_id".to_string(), Value::String(media_id));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let media_file = result
        .first()
        .and_then(|rows| rows.first())
        .ok_or(StatusCode::NOT_FOUND)?;

    // Extract file metadata
    let file_path = media_file
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let content_type = media_file
        .get("content_type")
        .and_then(|v| v.as_str())
        .unwrap_or("application/octet-stream");
    
    let content_length = media_file
        .get("content_length")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let upload_name = media_file
        .get("upload_name")
        .and_then(|v| v.as_str());

    // Open file for reading
    let file = fs::File::open(file_path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // Build response with appropriate headers and security headers
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

    // Add Content-Disposition header if upload name is available
    if let Some(name) = upload_name {
        response = response.header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", name)
        );
    }

    response
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}