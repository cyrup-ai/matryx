use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};

use matryx_entity::DeviceKeys;
use serde::{Deserialize, Serialize};
use serde_json;
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::{RepositoryError, device::DeviceRepository};

/// Extract device ID from Matrix authentication
fn extract_device_id_from_auth(auth: &MatrixAuth) -> Option<String> {
    match auth {
        MatrixAuth::User(token_info) => Some(token_info.device_id.clone()),
        _ => None,
    }
}

/// Device trust level enumeration
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub enum TrustLevel {
    #[serde(rename = "unverified")]
    #[default]
    Unverified,
    #[serde(rename = "cross_signed")]
    CrossSigned,
    #[serde(rename = "verified")]
    Verified,
    #[serde(rename = "blacklisted")]
    Blacklisted,
}

/// Client API Device Information - different from federation DeviceInfo
#[derive(Serialize, Clone, Debug)]
pub struct ClientDeviceInfo {
    pub device_id: String,
    pub display_name: Option<String>,
    pub last_seen_ip: Option<String>,
    pub last_seen_ts: Option<u64>,
    pub user_id: String,
    pub created_ts: u64,
    pub device_keys: Option<DeviceKeys>,
    pub trust_level: TrustLevel,
    pub is_deleted: bool,
}

/// Matrix Client-Server API v1.11 devices list response
#[derive(Serialize)]
pub struct DevicesResponse {
    pub devices: Vec<ClientDeviceInfo>,
}

/// Device registration request
#[derive(Deserialize)]
pub struct DeviceRegistrationRequest {
    pub device_id: String,
    pub initial_device_display_name: Option<String>,
    pub initial_device_keys: Option<DeviceKeys>,
}

/// Device registration response
#[derive(Serialize)]
pub struct DeviceRegistrationResponse {
    pub device_id: String,
    pub access_token: String,
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
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Device list failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match &auth {
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

    // Update current device activity
    if let Some(current_device_id) = extract_device_id_from_auth(&auth) {
        let _ = device_repo
            .update_device_activity(&current_device_id, &user_id, Some(addr.ip().to_string()))
            .await;
    }

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
    let device_infos: Vec<ClientDeviceInfo> = devices
        .into_iter()
        .map(|device| {
            ClientDeviceInfo {
                device_id: device.device_id.clone(),
                display_name: device.display_name.clone(),
                last_seen_ip: device.last_seen_ip.clone(),
                last_seen_ts: device.last_seen_ts.map(|ts| ts as u64),
                user_id: device.user_id.clone(),
                created_ts: device.created_at.timestamp() as u64,
                device_keys: device.device_keys.and_then(|keys| serde_json::from_value(keys).ok()),
                trust_level: TrustLevel::default(),
                is_deleted: false,
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

/// Enhanced device registration with automatic key setup
pub async fn register_device_with_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeviceRegistrationRequest>,
) -> Result<Json<DeviceRegistrationResponse>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => token_info.user_id.clone(),
        _ => return Err(StatusCode::FORBIDDEN),
    };

    let device_repo = DeviceRepository::new(state.db.clone());

    // Create device with initial metadata
    let device_info = matryx_entity::types::Device {
        device_id: request.device_id.clone(),
        user_id: user_id.clone(),
        display_name: request.initial_device_display_name.clone(),
        last_seen_ip: None,
        last_seen_ts: Some(chrono::Utc::now().timestamp()),
        created_at: chrono::Utc::now(),
        hidden: Some(false),
        device_keys: request
            .initial_device_keys
            .as_ref()
            .and_then(|k| serde_json::to_value(k).ok()),
        one_time_keys: None,
        fallback_keys: None,
        user_agent: None,
        initial_device_display_name: request.initial_device_display_name.clone(),
    };

    let initial_keys = request.initial_device_keys.map(|keys| {
        matryx_entity::types::DeviceKey {
            user_id: keys.user_id,
            device_id: keys.device_id,
            algorithms: keys.algorithms,
            keys: keys.keys,
            signatures: keys.signatures,
            unsigned: None,
        }
    });

    let created_device = device_repo
        .create_device_with_metadata(device_info, initial_keys)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate access token (placeholder implementation)
    let access_token = format!("syt_{}_{}", user_id, chrono::Utc::now().timestamp());

    info!("Device registration completed for user: {} device: {}", user_id, request.device_id);

    Ok(Json(DeviceRegistrationResponse { device_id: created_device.device_id, access_token }))
}

pub mod by_device_id;
