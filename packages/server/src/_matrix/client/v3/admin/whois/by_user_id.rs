use axum::extract::ConnectInfo;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};

use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, extract_matrix_auth};
use crate::state::AppState;
use matryx_surrealdb::repository::{UserRepository, DeviceRepository, SessionRepository};

#[derive(Serialize)]
pub struct WhoisResponse {
    pub user_id: String,
    pub devices: HashMap<String, WhoisDeviceInfo>,
}

#[derive(Serialize)]
pub struct WhoisDeviceInfo {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub connections: Vec<ConnectionInfo>,
}

#[derive(Serialize)]
pub struct ConnectionInfo {
    pub ip: String,
    pub last_seen: u64,
    pub user_agent: Option<String>,
}

/// GET /_matrix/client/v3/admin/whois/{userId}
pub async fn get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(target_user_id): Path<String>,
) -> Result<Json<WhoisResponse>, StatusCode> {
    // Authenticate user
    let auth_result = extract_matrix_auth(&headers, &state.session_service).await;
    let matrix_auth = match auth_result {
        Ok(auth) => auth,
        Err(MatrixAuthError::MissingToken) => return Err(StatusCode::UNAUTHORIZED),
        Err(MatrixAuthError::MissingAuthorization) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let user_id = match matrix_auth {
        crate::auth::MatrixAuth::User(user_auth) => user_auth.user_id,
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    info!(
        "Admin whois request from user {} at {} for target user {}",
        user_id, addr, target_user_id
    );

    // Check if user is admin using repository
    let user_repo = UserRepository::new(state.db.clone());
    let is_admin = match user_repo.is_admin(&user_id).await {
        Ok(admin_status) => admin_status,
        Err(e) => {
            error!("Failed to check admin status: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    if !is_admin {
        error!("User {} attempted admin whois without admin privileges", user_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if target user exists using repository
    let user_exists = match user_repo.user_exists_admin(&target_user_id).await {
        Ok(exists) => exists,
        Err(e) => {
            error!("Failed to check user existence: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    if !user_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get user's devices using repository
    let device_repo = DeviceRepository::new(state.db.clone());
    let devices = match device_repo.get_user_devices_for_admin(&target_user_id).await {
        Ok(devices) => devices,
        Err(e) => {
            error!("Failed to query devices: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Build device info map
    let mut device_info_map = HashMap::new();

    for (device_id, _display_name, last_seen_ip, last_seen_ts, user_agent) in devices {
        let mut connections = Vec::new();

        if let (Some(ip), Some(ts)) = (last_seen_ip, last_seen_ts) {
            connections.push(ConnectionInfo { ip, last_seen: ts as u64, user_agent });
        }

        let sessions = vec![SessionInfo { connections }];

        device_info_map.insert(device_id, WhoisDeviceInfo { sessions });
    }

    // Get additional session information from access tokens using repository
    let session_repo = SessionRepository::new(state.db.clone());
    if let Ok(tokens) = session_repo.get_user_access_tokens_for_admin(&target_user_id).await {
        for (device_id, last_used_ip, last_used_ts) in tokens {
            if let Some(device_info) = device_info_map.get_mut(&device_id)
                && let (Some(ip), Some(ts)) = (last_used_ip, last_used_ts) {
                    // Add or update connection info
                    if let Some(session) = device_info.sessions.get_mut(0) {
                        // Check if we already have this IP, if not add it
                        let has_ip = session.connections.iter().any(|c| c.ip == ip);
                        if !has_ip {
                            session.connections.push(ConnectionInfo {
                                ip,
                                last_seen: ts as u64,
                                user_agent: None,
                            });
                        }
                    }
                }
        }
    }

    let response = WhoisResponse { user_id: target_user_id, devices: device_info_map };

    info!("Admin whois completed for user {}", response.user_id);

    Ok(Json(response))
}
