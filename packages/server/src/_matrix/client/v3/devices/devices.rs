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
    _matrix::client::v3::devices::{ClientDeviceInfo, TrustLevel},
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::{
    RepositoryError,
    device::DeviceRepository,
};

/// Matrix Client-Server API v1.11 device list response
#[derive(Serialize)]
pub struct DevicesResponse {
    devices: Vec<ClientDeviceInfo>,
}

/// Matrix Client-Server API v1.11 device creation request
#[derive(Deserialize)]
pub struct DeviceCreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,
}

/// Matrix Client-Server API v1.11 Section 5.5.1
///
/// GET /_matrix/client/v3/devices
///
/// Get a list of all devices for the current user. This returns information
/// about all devices associated with the user's account.
///
/// This endpoint requires authentication and will only return devices
/// belonging to the authenticated user.
pub async fn get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<DevicesResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
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

    info!(
        "Processing device list request for user: {} from: {}",
        user_id, addr
    );

    // Create device repository
    let device_repo = DeviceRepository::new(state.db.clone());

    // Get all user devices
    let devices = match device_repo.get_user_devices_list(&user_id).await {
        Ok(devices) => devices,
        Err(e) => {
            error!("Failed to get devices for user {}: {}", user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    let duration = start_time.elapsed();
    info!(
        "Device list completed successfully for user: {} count: {} duration: {:?}",
        user_id, devices.len(), duration
    );

    // Convert to Matrix API format
    let device_infos: Vec<ClientDeviceInfo> = devices
        .into_iter()
        .map(|device| ClientDeviceInfo {
            device_id: device.device_id.clone(),
            display_name: device.display_name,
            last_seen_ip: device.last_seen_ip,
            last_seen_ts: device.last_seen_ts.map(|ts| ts as u64),
            user_id: user_id.to_string(),
            created_ts: device.created_at.timestamp() as u64,
            device_keys: device.device_keys.and_then(|v| serde_json::from_value(v).ok()),
            trust_level: TrustLevel::Unverified, // Default trust level
            is_deleted: device.hidden.unwrap_or(false), // Use hidden field as is_deleted
        })
        .collect();

    Ok(Json(DevicesResponse {
        devices: device_infos,
    }))
}

/// Matrix Client-Server API v1.11 Section 5.5.5
///
/// POST /_matrix/client/v3/devices
///
/// Create a new device for the current user. This registers a new device
/// and returns device information including the device ID.
///
/// This endpoint requires authentication and will create a device
/// associated with the authenticated user.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<DeviceCreateRequest>,
) -> Result<Json<ClientDeviceInfo>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Device creation failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Device creation failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Device creation failed - server authentication not allowed for device creation");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Device creation failed - anonymous authentication not allowed for device creation");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing device creation request for user: {} from: {}",
        user_id, addr
    );

    // Create device repository
    let device_repo = DeviceRepository::new(state.db.clone());

    // Create new device
    let device = match device_repo.create_device(
        &user_id,
        request.initial_device_display_name.as_deref(),
        Some(&addr.ip().to_string()),
        None, // No device keys initially
    ).await {
        Ok(device) => device,
        Err(e) => {
            error!("Failed to create device for user {}: {}", user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    let duration = start_time.elapsed();
    info!(
        "Device creation completed successfully for user: {} device: {} duration: {:?}",
        user_id, device.device_id, duration
    );

    // Log security event for device creation
    info!(
        "Security event: device_creation user: {} device: {} from ip: {} at {}",
        user_id,
        device.device_id,
        addr,
        chrono::Utc::now().timestamp()
    );

    // Convert to Matrix API format
    let device_info = ClientDeviceInfo {
        device_id: device.device_id.clone(),
        display_name: device.display_name,
        last_seen_ip: device.last_seen_ip,
        last_seen_ts: device.last_seen_ts.map(|ts| ts as u64),
        user_id: user_id.to_string(),
        created_ts: device.created_at.timestamp() as u64,
        device_keys: device.device_keys.and_then(|v| serde_json::from_value(v).ok()),
        trust_level: TrustLevel::Unverified, // Default trust level
        is_deleted: device.hidden.unwrap_or(false), // Use hidden field as is_deleted
    };

    Ok(Json(device_info))
}

/// Matrix Client-Server API v1.11 Section 5.5.6
///
/// DELETE /_matrix/client/v3/devices
///
/// Delete multiple devices for the current user. This will invalidate all
/// access tokens associated with the devices and sign out all the devices.
///
/// This endpoint requires authentication and additional verification may be
/// required to delete devices.
pub async fn delete(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(device_ids): Json<Vec<String>>,
) -> Result<Json<Value>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Multiple device deletion failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let (user_id, current_device_id) = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Multiple device deletion failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            (token_info.user_id.clone(), token_info.device_id.clone())
        },
        MatrixAuth::Server(_) => {
            warn!("Multiple device deletion failed - server authentication not allowed");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Multiple device deletion failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing multiple device deletion request for user: {} devices: {:?} from: {}",
        user_id, device_ids, addr
    );

    // Prevent self-deletion (deleting the device used to make this request)
    if device_ids.contains(&current_device_id) {
        warn!(
            "Multiple device deletion failed - cannot delete current device {} for user: {}",
            current_device_id, user_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Create device repository
    let device_repo = DeviceRepository::new(state.db.clone());

    // Delete all specified devices
    let mut deleted_count = 0;
    for device_id in &device_ids {
        // Verify device exists and belongs to user
        match device_repo.get_by_user_and_device(&user_id, device_id).await {
            Ok(Some(_)) => {
                // Delete the device
                if let Err(e) = device_repo.delete(device_id).await {
                    error!("Failed to delete device {} for user {}: {}", device_id, user_id, e);
                    continue;
                }
                deleted_count += 1;
            },
            Ok(None) => {
                warn!("Device {} not found for user: {}", device_id, user_id);
                continue;
            },
            Err(e) => {
                error!("Failed to verify device {} for user {}: {}", device_id, user_id, e);
                continue;
            },
        }
    }

    let duration = start_time.elapsed();
    info!(
        "Multiple device deletion completed for user: {} deleted: {}/{} duration: {:?}",
        user_id, deleted_count, device_ids.len(), duration
    );

    // Log security event for multiple device deletion
    info!(
        "Security event: multiple_device_deletion user: {} devices: {:?} from ip: {} at {}",
        user_id,
        device_ids,
        addr,
        chrono::Utc::now().timestamp()
    );

    // Return empty JSON object as per Matrix spec
    Ok(Json(json!({})))
}