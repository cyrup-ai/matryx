use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use matryx_entity::DeviceInfo;
use serde::{Deserialize, Serialize};

use tracing::{debug, error, warn};

use crate::AppState;
use matryx_surrealdb::repository::{UserRepository, DeviceRepository, CrossSigningRepository};

/// Cross-signing key structure
#[derive(Debug, Serialize, Deserialize)]
pub struct CrossSigningKey {
    pub keys: std::collections::HashMap<String, String>,
    pub signatures:
        Option<std::collections::HashMap<String, std::collections::HashMap<String, String>>>,
    pub usage: Vec<String>,
    pub user_id: String,
}

/// Response structure for device list endpoint
#[derive(Debug, Serialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceInfo>,
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub stream_id: i64,
    pub user_id: String,
}

/// GET /_matrix/federation/v1/user/devices/{userId}
///
/// Returns the current devices and identity keys for the given user.
/// This is used for initial device list population and resynchronization.
pub async fn get(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<DeviceListResponse>, StatusCode> {
    // Validate X-Matrix authentication header
    let _origin_server = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
    })?;

    debug!("Federation device list request for user: {}", user_id);

    // Verify the user exists locally
    let user_repo = UserRepository::new(state.db.clone());
    let user_exists = user_repo.user_exists(&user_id).await.map_err(|e| {
        error!("Failed to verify user existence: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !user_exists {
        warn!("Device list requested for non-existent user: {}", user_id);
        return Err(StatusCode::NOT_FOUND);
    }

    // Query all devices for the user
    let device_repo = DeviceRepository::new(state.db.clone());
    let federation_devices = device_repo.get_user_devices_for_federation(&user_id).await.map_err(|e| {
        error!("Failed to get user devices for federation: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut devices = Vec::new();
    for fed_device in federation_devices {
        if let Some(repo_device_keys) = fed_device.device_keys {
            // Convert repository DeviceKeys to entity DeviceKeys
            let entity_device_keys = matryx_entity::types::DeviceKeys {
                user_id: repo_device_keys.user_id,
                device_id: repo_device_keys.device_id,
                algorithms: repo_device_keys.algorithms,
                keys: repo_device_keys.keys,
                signatures: repo_device_keys.signatures,
                unsigned: repo_device_keys.unsigned,
            };
            
            let device_info = DeviceInfo {
                device_display_name: fed_device.display_name,
                device_id: fed_device.device_id,
                keys: entity_device_keys,
            };
            devices.push(device_info);
        }
    }

    // Query cross-signing keys
    let cross_signing_repo = CrossSigningRepository::new(state.db.clone());
    let (master_key, self_signing_key) = match get_federation_cross_signing_keys(&cross_signing_repo, &user_id).await {
        Ok((master, self_signing)) => (master, self_signing),
        Err(e) => {
            error!("Failed to query cross-signing keys for user {}: {}", user_id, e);
            (None, None)
        },
    };

    // Get current stream_id for this user's device updates
    let stream_id = get_device_stream_id(&state, &user_id).await.unwrap_or(0);

    debug!("Returning device list for user {} with {} devices", user_id, devices.len());

    Ok(Json(DeviceListResponse {
        devices,
        master_key,
        self_signing_key,
        stream_id,
        user_id,
    }))
}

/// Parse X-Matrix authentication header for federation
fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<String, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if !auth_header.starts_with("X-Matrix ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_params = &auth_header[9..]; // Skip "X-Matrix "

    // Parse origin parameter
    for param in auth_params.split(',') {
        let param = param.trim();
        if let Some((key, value)) = param.split_once('=')
            && key.trim() == "origin" {
            return Ok(value.trim().to_string());
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

/// Get cross-signing keys for federation from repository
async fn get_federation_cross_signing_keys(
    cross_signing_repo: &CrossSigningRepository,
    user_id: &str,
) -> Result<
    (Option<CrossSigningKey>, Option<CrossSigningKey>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let master_key = cross_signing_repo.get_master_key(user_id).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    
    let self_signing_key = cross_signing_repo.get_self_signing_key(user_id).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    // Convert repository types to federation types
    let fed_master_key = master_key.map(|k| CrossSigningKey {
        keys: k.keys,
        signatures: k.signatures,
        usage: k.usage,
        user_id: k.user_id,
    });

    let fed_self_signing_key = self_signing_key.map(|k| CrossSigningKey {
        keys: k.keys,
        signatures: k.signatures,
        usage: k.usage,
        user_id: k.user_id,
    });

    Ok((fed_master_key, fed_self_signing_key))
}

/// Get the current device stream ID for a user (simplified implementation)
async fn get_device_stream_id(
    _state: &AppState,
    _user_id: &str,
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    // For now, return a simple stream ID
    // In production, this would track actual device list update streams
    Ok(1)
}
