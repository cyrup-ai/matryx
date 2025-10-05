use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    _matrix::client::v3::devices::{ClientDeviceInfo, TrustLevel},
    AppState,
    auth::{
        MatrixAuth, extract_matrix_auth,
        uia::{UiaAuth, UiaFlow},
    },
};
use matryx_surrealdb::repository::{
    RepositoryError, device::DeviceRepository, session::SessionRepository,
};

// DeviceInfo already imported above

/// Matrix Client-Server API v1.11 device update request
#[derive(Deserialize)]
pub struct DeviceUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Matrix Client-Server API v1.11 device deletion request
#[derive(Deserialize)]
pub struct DeviceDeleteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Value>, // Authentication data for device deletion
}

/// Matrix Client-Server API v1.11 Section 5.5.4
///
/// DELETE /_matrix/client/v3/devices/{deviceId}
///
/// Delete a specific device for the current user. This will invalidate all
/// access tokens associated with the device and sign out the device.
///
/// This endpoint requires authentication and additional verification may be
/// required to delete devices.
pub async fn delete(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    Json(request): Json<DeviceDeleteRequest>,
) -> Result<Json<Value>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Device deletion failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let (user_id, current_device_id) = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Device deletion failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            (token_info.user_id.clone(), token_info.device_id.clone())
        },
        MatrixAuth::Server(_) => {
            warn!("Device deletion failed - server authentication not allowed for device deletion");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!(
                "Device deletion failed - anonymous authentication not allowed for device deletion"
            );
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    // Handle User-Interactive Authentication per Matrix specification
    // Device deletion is a sensitive operation that requires proper authentication
    if let Some(auth_data) = request.auth {
        info!("Processing UIA authentication for device deletion");

        // Parse auth data as UIA authentication
        let uia_auth: UiaAuth = serde_json::from_value(auth_data).map_err(|e| {
            error!("Failed to parse UIA auth data for device deletion: {:?}", e);
            StatusCode::BAD_REQUEST
        })?;

        // Use centralized UIA service from AppState
        let uia_service = &state.uia_service;

        // Validate the authentication
        let session_id = uia_auth.session.clone().unwrap_or_default();
        match uia_service.process_auth(&session_id, uia_auth).await {
            Ok(_) => {
                info!("UIA authentication successful for device deletion");
                // Authentication passed, continue with device deletion
            },
            Err(uia_error) => {
                warn!("UIA authentication failed for device deletion: {:?}", uia_error);
                // Return UIA error response per Matrix spec
                return Ok(Json(json!({
                    "flows": uia_error.flows,
                    "params": uia_error.params,
                    "session": uia_error.session,
                    "completed": uia_error.completed,
                    "error": uia_error.error,
                    "errcode": uia_error.errcode
                })));
            },
        }
    } else {
        // No auth data provided - start UIA flow per Matrix spec
        // Device deletion is sensitive and requires authentication
        info!("No UIA auth provided for device deletion, starting UIA flow");

        // Use centralized UIA service from AppState
        let uia_service = &state.uia_service;

        // Define required authentication flows for device deletion
        let flows = vec![UiaFlow { stages: vec!["m.login.password".to_string()] }];

        // Start UIA session
        let session = uia_service
            .start_session(
                Some(&user_id),
                Some(&device_id), // device_id relevant for device operations
                flows.clone(),
                std::collections::HashMap::new(),
            )
            .await
            .map_err(|e| {
                error!("Failed to start UIA session for device deletion: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Return UIA challenge per Matrix spec
        return Ok(Json(json!({
            "flows": flows,
            "params": {},
            "session": session.session_id,
            "completed": [],
            "error": "User Interactive Authentication required for device deletion",
            "errcode": "M_FORBIDDEN"
        })));
    }

    info!(
        "Processing device deletion request for user: {} device: {} from: {}",
        user_id, device_id, addr
    );

    // Prevent self-deletion (deleting the device used to make this request)
    if device_id == current_device_id {
        warn!(
            "Device deletion failed - cannot delete current device {} for user: {}",
            device_id, user_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Create repositories
    let device_repo = DeviceRepository::new(state.db.clone());
    let session_repo = SessionRepository::new(state.db.clone());

    // Verify device exists and belongs to user
    let device = match device_repo.get_by_user_and_device(&user_id, &device_id).await {
        Ok(Some(device)) => device,
        Ok(None) => {
            warn!("Device deletion failed - device {} not found for user: {}", device_id, user_id);
            return Err(StatusCode::NOT_FOUND);
        },
        Err(e) => {
            error!("Failed to retrieve device {} for user {}: {}", device_id, user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Delete all sessions for this device (sign out the device)
    if let Err(e) = session_repo.delete_by_user(&user_id).await {
        match e {
            RepositoryError::NotFound { .. } => {
                info!("No active sessions found for device {} user: {}", device_id, user_id);
            },
            _ => {
                error!(
                    "Failed to invalidate sessions for device {} user {}: {}",
                    device_id, user_id, e
                );
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            },
        }
    }

    // Delete the device record
    if let Err(e) = device_repo.delete(&device.device_id).await {
        error!("Failed to delete device {} for user {}: {}", device_id, user_id, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let duration = start_time.elapsed();
    info!(
        "Device deletion completed successfully for user: {} device: {} duration: {:?}",
        user_id, device_id, duration
    );

    // Log security event for device deletion
    info!(
        "Security event: device_deletion user: {} device: {} from ip: {} at {}",
        user_id,
        device_id,
        addr,
        chrono::Utc::now().timestamp()
    );

    // Return empty JSON object as per Matrix spec
    Ok(Json(json!({})))
}

/// Matrix Client-Server API v1.11 Section 5.5.2
///
/// GET /_matrix/client/v3/devices/{deviceId}
///
/// Get information about a specific device for the current user. This returns
/// detailed information about the device including display name and last seen data.
///
/// This endpoint requires authentication and will only return information for
/// devices belonging to the authenticated user.
pub async fn get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<Json<ClientDeviceInfo>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Device info failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Device info failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Device info failed - server authentication not allowed for device info");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Device info failed - anonymous authentication not allowed for device info");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing device info request for user: {} device: {} from: {}",
        user_id, device_id, addr
    );

    // Create device repository
    let device_repo = DeviceRepository::new(state.db.clone());

    // Get the specific device
    let device = match device_repo.get_by_user_and_device(&user_id, &device_id).await {
        Ok(Some(device)) => device,
        Ok(None) => {
            warn!("Device info failed - device {} not found for user: {}", device_id, user_id);
            return Err(StatusCode::NOT_FOUND);
        },
        Err(e) => {
            error!("Failed to retrieve device {} for user {}: {}", device_id, user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    let duration = start_time.elapsed();
    info!(
        "Device info completed successfully for user: {} device: {} duration: {:?}",
        user_id, device_id, duration
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

/// Matrix Client-Server API v1.11 Section 5.5.3
///
/// PUT /_matrix/client/v3/devices/{deviceId}
///
/// Update information about a specific device for the current user. This typically
/// involves updating the device's display name for easier identification.
///
/// This endpoint requires authentication and will only allow updates to devices
/// belonging to the authenticated user.
pub async fn put(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    Json(request): Json<DeviceUpdateRequest>,
) -> Result<Json<Value>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Device update failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Device update failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Device update failed - server authentication not allowed for device update");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Device update failed - anonymous authentication not allowed for device update");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing device update request for user: {} device: {} from: {}",
        user_id, device_id, addr
    );

    // Create device repository
    let device_repo = DeviceRepository::new(state.db.clone());

    // Get the existing device to update
    let mut device = match device_repo.get_by_user_and_device(&user_id, &device_id).await {
        Ok(Some(device)) => device,
        Ok(None) => {
            warn!("Device update failed - device {} not found for user: {}", device_id, user_id);
            return Err(StatusCode::NOT_FOUND);
        },
        Err(e) => {
            error!("Failed to retrieve device {} for user {}: {}", device_id, user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Update the device display name if provided
    if let Some(display_name) = request.display_name {
        device.display_name = if display_name.is_empty() {
            None
        } else {
            Some(display_name)
        };

        // Save the updated device
        if let Err(e) = device_repo.update(&device).await {
            error!("Failed to update device {} for user {}: {}", device_id, user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let duration = start_time.elapsed();
    info!(
        "Device update completed successfully for user: {} device: {} duration: {:?}",
        user_id, device_id, duration
    );

    // Return empty JSON object as per Matrix spec
    Ok(Json(json!({})))
}
