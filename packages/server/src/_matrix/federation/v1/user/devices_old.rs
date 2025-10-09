use axum::{
    Json,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{AppState, federation::device_management::CrossSigningKey};
use crate::auth::verify_x_matrix_auth;
use crate::utils::request_helpers::extract_request_uri;
use matryx_entity::types::Device;
use matryx_surrealdb::repository::device::DeviceRepository;
use matryx_surrealdb::repository::{CrossSigningRepository, EDURepository};

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

/// GET /_matrix/federation/v1/user/devices/{userId}
pub async fn get_user_devices_federation(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Result<Json<UserDevicesResponse>, StatusCode> {
    // Verify X-Matrix authentication using actual request URI
    let uri = extract_request_uri(&request);
    let auth_result = verify_x_matrix_auth(
        &headers,
        &state.homeserver_name,
        "GET",
        uri,
        None, // No body for GET requests
        state.event_signer.get_signing_engine(),
    ).await;
    
    let x_matrix_auth = auth_result.map_err(|e| {
        error!("X-Matrix authentication failed for devices query: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    info!("Federation device request authenticated from: {}", x_matrix_auth.origin);

    // Verify user belongs to local server
    if !is_local_user(&user_id, &state.homeserver_name) {
        return Err(StatusCode::NOT_FOUND);
    }

    let device_repo = DeviceRepository::new(state.db.clone());
    let devices = device_repo.get_by_user(&user_id).await.map_err(|e| {
        error!("Failed to retrieve devices for federation request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get cross-signing keys
    let cross_signing_repo = CrossSigningRepository::new(state.db.clone());
    let master_key = cross_signing_repo.get_master_key(&user_id).await.map_err(|e| {
        error!("Failed to retrieve master key for federation request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let self_signing_key = cross_signing_repo.get_self_signing_key(&user_id).await.map_err(|e| {
        error!("Failed to retrieve self-signing key for federation request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get current stream ID for this user
    let edu_repo = EDURepository::new(state.db.clone());
    let stream_id = edu_repo.get_latest_device_list_stream_id(&user_id).await.map_err(|e| {
        error!("Failed to retrieve device list stream ID for federation request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.unwrap_or(0) as i64;

    info!("Federation device request completed for user: {} ({} devices)", user_id, devices.len());

    Ok(Json(UserDevicesResponse {
        user_id,
        devices,
        master_key,
        self_signing_key,
        stream_id,
    }))
}
