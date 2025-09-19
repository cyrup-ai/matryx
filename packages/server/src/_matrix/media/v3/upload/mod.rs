use axum::{
    body::Bytes,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::{
    auth::MatrixSessionService,
    config::ServerConfig,
    database::SurrealRepository,
    AppState,
};

#[derive(Serialize)]
pub struct MediaUploadResponse {
    pub content_uri: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MediaFile {
    pub media_id: String,
    pub server_name: String,
    pub content_type: String,
    pub content_length: u64,
    pub file_path: String,
    pub upload_name: Option<String>,
    pub uploaded_by: String,
    pub created_at: String,
}

fn validate_content_type(content_type: &str) -> bool {
    // Allow common media types
    matches!(content_type, 
        "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/svg+xml" |
        "video/mp4" | "video/webm" | "video/ogg" |
        "audio/mp3" | "audio/ogg" | "audio/wav" | "audio/flac" |
        "application/pdf" | "text/plain" | "application/json" |
        "application/octet-stream"
    )
}

fn get_file_extension(content_type: &str) -> &'static str {
    match content_type {
        "image/jpeg" => "jpg",
        "image/png" => "png", 
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/ogg" => "ogv",
        "audio/mp3" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" => "wav",
        "audio/flac" => "flac",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "application/json" => "json",
        _ => "bin",
    }
}

pub async fn upload_media(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<MediaUploadResponse>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Process multipart upload
    let mut file_data: Option<Bytes> = None;
    let mut content_type = String::from("application/octet-stream");
    let mut upload_name: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        if let Some(field_name) = field.name() {
            match field_name {
                "file" => {
                    // Get content type from field
                    if let Some(field_content_type) = field.content_type() {
                        content_type = field_content_type.to_string();
                    }
                    
                    // Get filename if provided
                    if let Some(filename) = field.file_name() {
                        upload_name = Some(filename.to_string());
                    }
                    
                    // Read file data
                    let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                    file_data = Some(data);
                }
                _ => {
                    // Skip unknown fields
                    let _ = field.bytes().await;
                }
            }
        }
    }

    let file_bytes = file_data.ok_or(StatusCode::BAD_REQUEST)?;
    
    // Validate file size (50MB limit)
    const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;
    if file_bytes.len() > MAX_FILE_SIZE {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    // Validate content type
    if !validate_content_type(&content_type) {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    // Generate media ID and file path
    let media_id = Uuid::new_v4().to_string();
    let server_name = &ServerConfig::get().homeserver_name;
    let file_extension = get_file_extension(&content_type);
    
    // Create media directory structure
    let media_dir = PathBuf::from("media").join(&server_name);
    fs::create_dir_all(&media_dir).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let file_path = media_dir.join(format!("{}.{}", media_id, file_extension));
    
    // Write file to disk
    let mut file = fs::File::create(&file_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    file.write_all(&file_bytes).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    file.sync_all().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Store media metadata in database
    let query = r#"
        CREATE media_files SET
            media_id = $media_id,
            server_name = $server_name,
            content_type = $content_type,
            content_length = $content_length,
            file_path = $file_path,
            upload_name = $upload_name,
            uploaded_by = $uploaded_by,
            created_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("media_id".to_string(), Value::String(media_id.clone()));
    params.insert("server_name".to_string(), Value::String(server_name.clone()));
    params.insert("content_type".to_string(), Value::String(content_type));
    params.insert("content_length".to_string(), Value::Number(serde_json::Number::from(file_bytes.len())));
    params.insert("file_path".to_string(), Value::String(file_path.to_string_lossy().to_string()));
    params.insert("upload_name".to_string(), upload_name.map(Value::String).unwrap_or(Value::Null));
    params.insert("uploaded_by".to_string(), Value::String(token_info.user_id));

    state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Return MXC URI
    let content_uri = format!("mxc://{}/{}", server_name, media_id);
    
    Ok(Json(MediaUploadResponse {
        content_uri,
    }))
}