pub mod by_server_name;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};

use image::{DynamicImage, ImageFormat, imageops::FilterType};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use tokio::fs;
use tokio_util::io::ReaderStream;

use crate::AppState;
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

async fn generate_thumbnail(
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
) -> Result<Response<Body>, StatusCode> {
    // Validate thumbnail dimensions
    if query.width == 0 || query.height == 0 || query.width > 2048 || query.height > 2048 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Parse thumbnail method
    let method = ThumbnailMethod::from_str(&query.method).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Generate thumbnail using MediaService
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, &query.method)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let thumbnail_data = thumbnail_result.thumbnail;

    // Return thumbnail response
    let content_length = thumbnail_data.len();
    let body = Body::from(thumbnail_data);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, thumbnail_result.content_type)
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
