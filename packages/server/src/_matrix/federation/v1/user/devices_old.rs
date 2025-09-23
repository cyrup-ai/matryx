use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{AppState, federation::device_management::CrossSigningKey};
use matryx_entity::types::Device;
use matryx_surrealdb::repository::device::DeviceRepository;

#[derive(Serialize)]
pub struct UserDevicesResponse {
    pub user_id: String,
    pub devices: Vec<Device>,
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub stream_id: i64,
}

/// Helper function to check if user belongs to local server
fn is_local_user(user_id: &str, server_name: &str) -> bool {
    user_id
        .split(':')
        .nth(1)
        .map(|domain| domain == server_name)
        .unwrap_or(false)
}

/// Helper function to extract federation authentication (placeholder)
async fn extract_federation_auth(_headers: &HeaderMap, _federation_service: &()) -> Result<(), ()> {
    // Placeholder for federation authentication
    Ok(())
}

/// GET /_matrix/federation/v1/user/devices/{userId}
pub async fn get_user_devices_federation(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<UserDevicesResponse>, StatusCode> {
    // Verify federation authentication
    let _auth = extract_federation_auth(&headers, &())
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Verify user belongs to local server
    if !is_local_user(&user_id, "localhost") {
        // TODO: Get actual server name from config
        return Err(StatusCode::NOT_FOUND);
    }

    let device_repo = DeviceRepository::new(state.db.clone());
    let devices = device_repo.get_by_user(&user_id).await.map_err(|e| {
        error!("Failed to retrieve devices for federation request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get cross-signing keys (placeholder - would integrate with actual key repository)
    let master_key = None; // TODO: Implement cross-signing key retrieval
    let self_signing_key = None; // TODO: Implement cross-signing key retrieval

    // Get current stream ID for this user (placeholder)
    let stream_id = 0; // TODO: Implement stream ID retrieval

    info!("Federation device request completed for user: {} ({} devices)", user_id, devices.len());

    Ok(Json(UserDevicesResponse {
        user_id,
        devices,
        master_key,
        self_signing_key,
        stream_id,
    }))
}
