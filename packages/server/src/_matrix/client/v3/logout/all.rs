use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};

use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::{
    RepositoryError, device::DeviceRepository, session::SessionRepository,
};

/// Matrix Client-Server API v1.11 Section 5.4.2
///
/// POST /_matrix/client/v3/logout/all
///
/// Invalidates all access tokens for the user, effectively logging out all devices.
/// This includes the access token used to make the request.
///
/// This endpoint requires authentication and will return an empty JSON object on success.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(_request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Logout all failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let (user_id, current_device_id) = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Logout all failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            (token_info.user_id.clone(), token_info.device_id.clone())
        },
        MatrixAuth::Server(_) => {
            warn!("Logout all failed - server authentication not allowed for logout");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Logout all failed - anonymous authentication not allowed for logout");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing logout all request for user: {} from device: {} ip: {}",
        user_id, current_device_id, addr
    );

    // Create repositories for bulk token invalidation
    let session_repo = SessionRepository::new(state.db.clone());
    let device_repo = DeviceRepository::new(state.db.clone());

    let user_devices = match device_repo.get_by_user(&user_id).await {
        Ok(devices) => devices,
        Err(e) => {
            error!("Failed to retrieve user devices during logout all: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    let device_count = user_devices.len();
    info!("Found {} devices for user {} during logout all", device_count, user_id);

    // Invalidate all access tokens for the user (atomic bulk operation)
    if let Err(e) = session_repo.invalidate_all_tokens(&user_id).await {
        error!("Failed to invalidate all tokens during logout all: {}", e);
        return match e {
            RepositoryError::NotFound { .. } => {
                info!("No active tokens found for user: {} during logout all", user_id);
                Ok(Json(json!({})))
            },
            _ => {
                error!("Database error during logout all token invalidation: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            },
        };
    }

    // Log logout all event for security monitoring
    info!(
        "Security event: logout_all for user: {} initiating device: {} from ip: {} devices_logged_out: {} at {}",
        user_id,
        current_device_id,
        addr,
        device_count,
        chrono::Utc::now().timestamp()
    );

    let duration = start_time.elapsed();
    info!(
        "User logout all completed successfully for user: {} ({} devices) duration: {:?}",
        user_id, device_count, duration
    );

    // Return empty JSON object as per Matrix spec
    Ok(Json(json!({})))
}
