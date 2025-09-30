use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};

use serde_json::Value;
use tracing::{error, info, warn};

use crate::AppState;
use crate::auth::{MatrixAuth, extract_matrix_auth};

use matryx_surrealdb::repository::{DeviceRepository, SessionRepository};

#[derive(Debug)]
pub enum LogoutError {
    InvalidToken,
    SessionNotFound,
    DatabaseError,
    InternalError,
}

impl From<LogoutError> for StatusCode {
    fn from(error: LogoutError) -> Self {
        match error {
            LogoutError::InvalidToken => StatusCode::UNAUTHORIZED,
            LogoutError::SessionNotFound => StatusCode::UNAUTHORIZED,
            LogoutError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
            LogoutError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// POST /_matrix/client/v3/logout
///
/// Implements Matrix Client-Server API hard logout endpoint with complete session invalidation.
///
/// Features:
/// - Invalidates the current access token
/// - Deletes the associated device and device keys
/// - Cleans up session data from database
/// - Terminates any active LiveQuery subscriptions
/// - Full Matrix specification compliance
pub async fn post_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    info!("Processing hard logout request");
    logout_internal(state, headers, false).await
}

/// POST /_matrix/client/v3/logout/soft
///
/// Implements Matrix Client-Server API soft logout endpoint that preserves device information.
///
/// Features:
/// - Invalidates the current access token and refresh tokens
/// - Preserves device registration and E2EE keys
/// - Maintains device information for future logins
/// - Cleans up session data but keeps device record
/// - Allows seamless re-authentication without losing encryption keys
pub async fn post_soft_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    info!("Processing soft logout request");
    logout_internal(state, headers, true).await
}

/// Internal logout implementation supporting both hard and soft logout
async fn logout_internal(
    state: AppState,
    headers: HeaderMap,
    soft_logout: bool,
) -> Result<Json<Value>, StatusCode> {
    let logout_type = if soft_logout { "soft" } else { "hard" };

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("{} logout failed - authentication extraction failed: {}", logout_type, e);
        StatusCode::UNAUTHORIZED
    })?;

    // Check authentication and extract user information using MatrixAuth utility methods
    if auth.is_expired() {
        warn!("{} logout failed - access token expired for user", logout_type);
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (user_id, device_id, access_token) = match (auth.user_id(), auth.device_id(), auth.access_token()) {
        (Some(user_id), Some(device_id), Some(access_token)) => {
            (user_id.to_string(), device_id.to_string(), access_token.to_string())
        },
        _ => {
            warn!("{} logout failed - user authentication required", logout_type);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Ensure we have user authentication (not server or anonymous)
    match auth {
        MatrixAuth::User(_) => {
            // Valid user authentication - proceed with logout
        },
        MatrixAuth::Server(_) => {
            warn!("{} logout failed - server authentication not allowed for logout", logout_type);
            return Err(StatusCode::UNAUTHORIZED);
        },
        MatrixAuth::Anonymous => {
            warn!("{} logout failed - anonymous authentication not allowed for logout", logout_type);
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("{} logout request for user: {} device: {}", logout_type, user_id, device_id);

    // Initialize repositories for database operations
    let session_repo = SessionRepository::new(state.db.clone());
    let device_repo = DeviceRepository::new(state.db.clone());

    // Invalidate session in database
    invalidate_user_session(&session_repo, &user_id, &device_id, &access_token).await?;

    // Invalidate access token and refresh tokens
    invalidate_session_token(&session_repo, &access_token).await?;

    // Revoke all refresh tokens for this device
    revoke_device_refresh_tokens(&state, &user_id, &device_id).await?;

    if soft_logout {
        // Soft logout: preserve device and E2EE keys, only invalidate sessions
        preserve_device_for_soft_logout(&device_repo, &user_id, &device_id).await?;
        info!(
            "Soft logout successful for user: {} device: {} (device preserved)",
            user_id, device_id
        );
    } else {
        // Hard logout: delete device and associated device keys
        delete_user_device(&device_repo, &user_id, &device_id).await?;
        info!(
            "Hard logout successful for user: {} device: {} (device deleted)",
            user_id, device_id
        );
    }

    // Clean up any active LiveQuery subscriptions for this session
    cleanup_livequery_subscriptions(&state, &user_id, &device_id).await?;

    // Return empty JSON object as per Matrix specification
    Ok(Json(serde_json::json!({})))
}



/// Invalidate user session in database
async fn invalidate_user_session(
    session_repo: &SessionRepository,
    user_id: &str,
    device_id: &str,
    _access_token: &str,
) -> Result<(), LogoutError> {
    // Find and delete the session
    let session_id = format!("{}:{}", user_id, device_id);

    match session_repo.delete(&session_id).await {
        Ok(_) => {
            info!("Session invalidated for user: {} device: {}", user_id, device_id);
            Ok(())
        },
        Err(e) => {
            error!("Failed to invalidate session {}: {}", session_id, e);
            Err(LogoutError::DatabaseError)
        },
    }
}

/// Delete user device and associated device keys
async fn delete_user_device(
    device_repo: &DeviceRepository,
    user_id: &str,
    device_id: &str,
) -> Result<(), LogoutError> {
    let device_record_id = format!("{}:{}", user_id, device_id);

    match device_repo.delete(&device_record_id).await {
        Ok(_) => {
            info!("Device deleted for user: {} device: {}", user_id, device_id);
            Ok(())
        },
        Err(e) => {
            error!("Failed to delete device {}: {}", device_record_id, e);
            Err(LogoutError::DatabaseError)
        },
    }
}

/// Invalidate session token in database
async fn invalidate_session_token(
    session_repo: &SessionRepository,
    access_token: &str,
) -> Result<(), LogoutError> {
    match session_repo.invalidate_token(access_token).await {
        Ok(_) => {
            info!("Access token invalidated in database");
            Ok(())
        },
        Err(e) => {
            error!("Failed to invalidate token in database: {}", e);
            Err(LogoutError::DatabaseError)
        },
    }
}

/// Clean up LiveQuery subscriptions for the logged out session
async fn cleanup_livequery_subscriptions(
    state: &AppState,
    user_id: &str,
    device_id: &str,
) -> Result<(), LogoutError> {
    let session_repo = SessionRepository::new(state.db.clone());
    
    match session_repo.cleanup_livequery_subscriptions(user_id, device_id).await {
        Ok(_) => {
            info!("LiveQuery subscriptions cleaned up for user: {} device: {}", user_id, device_id);
            Ok(())
        },
        Err(e) => {
            error!("Failed to cleanup LiveQuery subscriptions: {}", e);
            // Don't fail the logout for cleanup errors, just log them
            Ok(())
        },
    }
}

/// Preserve device information for soft logout (mark as inactive but don't delete)
async fn preserve_device_for_soft_logout(
    device_repo: &DeviceRepository,
    user_id: &str,
    device_id: &str,
) -> Result<(), LogoutError> {
    // Use existing DeviceRepository to actually preserve the device
    match device_repo.get_by_id(device_id).await {
        Ok(Some(mut device)) => {
            // CRITICAL: Preserve E2EE keys - don't modify:
            // - device.device_keys (E2EE device identity)
            // - device.one_time_keys (for key exchange)
            // - device.fallback_keys (backup keys)

            // Update last seen timestamp for tracking
            device.last_seen_ts = Some(chrono::Utc::now().timestamp());

            // Update device in database with preserved E2EE keys
            device_repo.update(&device).await.map_err(|e| {
                error!("Failed to update device for soft logout: {}", e);
                LogoutError::DatabaseError
            })?;

            info!(
                "Device preserved for soft logout - user: {} device: {} (E2EE keys maintained)",
                user_id, device_id
            );
            Ok(())
        },
        Ok(None) => {
            warn!("Device not found for soft logout preservation: {}", device_id);
            Ok(()) // Don't fail logout if device not found
        },
        Err(e) => {
            error!("Database error during soft logout device preservation: {}", e);
            Err(LogoutError::DatabaseError)
        },
    }
}

/// Revoke all refresh tokens for a specific device
async fn revoke_device_refresh_tokens(
    state: &AppState,
    user_id: &str,
    device_id: &str,
) -> Result<(), LogoutError> {
    let session_repo = SessionRepository::new(state.db.clone());
    
    match session_repo.revoke_device_refresh_tokens(user_id, device_id).await {
        Ok(_) => {
            info!("Refresh tokens revoked for user: {} device: {}", user_id, device_id);
            Ok(())
        },
        Err(e) => {
            error!(
                "Failed to revoke refresh tokens for user: {} device: {}: {}",
                user_id, device_id, e
            );
            // Don't fail logout for refresh token revocation errors
            Ok(())
        },
    }
}


