use axum::{extract::State, http::StatusCode, response::Json};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Serialize)]
pub struct MediaConfigResponse {
    #[serde(rename = "m.upload.size")]
    pub upload_size: u64,
}

pub async fn get_media_config(
    State(state): State<AppState>,
) -> Result<Json<MediaConfigResponse>, StatusCode> {
    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Get media statistics for dynamic configuration
    let statistics = media_service
        .get_media_statistics(Some(&state.homeserver_name))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Calculate dynamic upload limit based on current usage
    // Base limit: 50MB, adjust based on storage usage
    let base_limit: u64 = 50 * 1024 * 1024; // 50MB

    // Reduce limit if storage usage is high (simplified calculation)
    let upload_size = if statistics.total_size > (1024 * 1024 * 1024) {
        // > 1GB
        base_limit / 2 // Reduce to 25MB if storage is high
    } else {
        base_limit
    };

    Ok(Json(MediaConfigResponse { upload_size }))
}

// HTTP method handler for main.rs routing
pub use get_media_config as get;
