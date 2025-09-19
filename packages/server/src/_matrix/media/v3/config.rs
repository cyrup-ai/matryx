use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct MediaConfigResponse {
    #[serde(rename = "m.upload.size")]
    pub upload_size: u64,
}

pub async fn get_media_config(
    State(_state): State<AppState>,
) -> Result<Json<MediaConfigResponse>, StatusCode> {
    // Return media configuration
    // 50MB upload limit (50 * 1024 * 1024 bytes)
    const MAX_UPLOAD_SIZE: u64 = 50 * 1024 * 1024;
    
    Ok(Json(MediaConfigResponse {
        upload_size: MAX_UPLOAD_SIZE,
    }))
}