use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::{RepositoryError, device::DeviceRepository};

/// Matrix Client-Server API v1.11 device information response
#[derive(Serialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ts: Option<u64>,
}

/// Matrix Client-Server API v1.11 devices list response  
#[derive(Serialize)]
pub struct DevicesResponse {
    pub devices: Vec<DeviceInfo>,
}

/// Matrix Client-Server API v1.11 Section 5.5.1
///
/// GET /_matrix/client/v3/devices
///
/// List all registered devices for the current user. This returns information
/// about all active devices associated with the user's account, including
/// display names and last seen timestamps.
///
/// This endpoint requires authentication and will return device information
/// for the authenticated user only.
pub async fn get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<DevicesResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Device list failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Device list failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Device list failed - server authentication not allowed for device list");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Device list failed - anonymous authentication not allowed for device list");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing device list request for user: {} from: {}", user_id, addr);

    // Create device repository
    let device_repo = DeviceRepository::new(state.db.clone());

    // Get all devices for the user
    let devices = match device_repo.get_by_user(&user_id).await {
        Ok(devices) => devices,
        Err(e) => {
            error!("Failed to retrieve devices for user {}: {}", user_id, e);
            return match e {
                RepositoryError::NotFound { .. } => {
                    // No devices found - return empty list
                    Ok(Json(DevicesResponse { devices: vec![] }))
                },
                _ => {
                    error!("Database error during device list retrieval: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                },
            };
        },
    };

    // Convert to Matrix API format
    let device_infos: Vec<DeviceInfo> = devices
        .into_iter()
        .map(|device| {
            DeviceInfo {
                device_id: device.device_id,
                display_name: device.display_name,
                last_seen_ip: device.last_seen_ip,
                last_seen_ts: device.last_seen_ts.map(|ts| ts as u64),
            }
        })
        .collect();

    let device_count = device_infos.len();
    let duration = start_time.elapsed();
    info!(
        "Device list completed successfully for user: {} ({} devices) duration: {:?}",
        user_id, device_count, duration
    );

    let response = DevicesResponse { devices: device_infos };

    Ok(Json(response))
}

pub mod by_device_id;
