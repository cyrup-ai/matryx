use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    body::Body,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio_util::io::ReaderStream;
use image::{ImageFormat, DynamicImage, imageops::FilterType};
use std::io::Cursor;

use crate::{
    database::SurrealRepository,
    AppState,
};

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
    matches!(content_type, 
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    )
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
        ThumbnailMethod::Crop => {
            img.resize_to_fill(width, height, FilterType::Lanczos3)
        },
        ThumbnailMethod::Scale => {
            img.resize(width, height, FilterType::Lanczos3)
        },
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
    let method = ThumbnailMethod::from_str(&query.method)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Query media metadata from database
    let media_query = "SELECT * FROM media_files WHERE server_name = $server_name AND media_id = $media_id";
    let mut params = HashMap::new();
    params.insert("server_name".to_string(), Value::String(server_name));
    params.insert("media_id".to_string(), Value::String(media_id));

    let result = state.database
        .query(media_query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let media_file = result
        .first()
        .and_then(|rows| rows.first())
        .ok_or(StatusCode::NOT_FOUND)?;

    // Extract file metadata
    let file_path_str = media_file
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let content_type = media_file
        .get("content_type")
        .and_then(|v| v.as_str())
        .unwrap_or("application/octet-stream");

    // Check if file is an image
    if !is_image_content_type(content_type) {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    let file_path = PathBuf::from(file_path_str);
    
    // Check if original file exists
    if !file_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Generate thumbnail cache path
    let cache_dir = PathBuf::from("media/thumbnails");
    fs::create_dir_all(&cache_dir).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let cache_filename = format!("{}_{}_{}_{}.jpg", 
        media_id, query.width, query.height, query.method);
    let cache_path = cache_dir.join(cache_filename);

    // Check if thumbnail already exists in cache
    let thumbnail_data = if cache_path.exists() {
        // Read cached thumbnail
        fs::read(&cache_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        // Generate new thumbnail
        let thumbnail_bytes = generate_thumbnail(&file_path, query.width, query.height, method)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // Cache the thumbnail
        fs::write(&cache_path, &thumbnail_bytes).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        thumbnail_bytes
    };

    // Return thumbnail response
    let body = Body::from(thumbnail_data);
    
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CONTENT_LENGTH, body.size_hint().lower().to_string())
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}