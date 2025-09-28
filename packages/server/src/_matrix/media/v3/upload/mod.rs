pub mod by_server_name;

use axum::{
    body::Bytes,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};




use crate::AppState;
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

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
    matches!(
        content_type,
        "image/jpeg" |
            "image/png" |
            "image/gif" |
            "image/webp" |
            "image/svg+xml" |
            "video/mp4" |
            "video/webm" |
            "video/ogg" |
            "audio/mp3" |
            "audio/ogg" |
            "audio/wav" |
            "audio/flac" |
            "application/pdf" |
            "text/plain" |
            "application/json" |
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
    let token_info = state
        .session_service
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
                },
                _ => {
                    // Skip unknown fields
                    let _ = field.bytes().await;
                },
            }
        }
    }

    let file_bytes = file_data.ok_or(StatusCode::BAD_REQUEST)?;

    // Validate content type before upload
    if !validate_content_type(&content_type) {
        warn!("Upload rejected - unsupported content type: {}", content_type);
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    // Generate appropriate file extension based on content type
    let file_extension = get_file_extension(&content_type);
    debug!("Uploading {} file with extension: {}", content_type, file_extension);

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Upload media using MediaService
    let upload_result = media_service
        .upload_media(&token_info.user_id, &file_bytes, &content_type, upload_name.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(MediaUploadResponse { content_uri: upload_result.content_uri }))
}

/// POST /_matrix/media/v3/upload
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<MediaUploadResponse>, StatusCode> {
    upload_media(State(state), headers, multipart).await
}
