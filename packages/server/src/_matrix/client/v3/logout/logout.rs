use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use serde_json::Value;
use tracing::{error, info, warn};

use crate::AppState;
use crate::auth::{MatrixAuth, extract_matrix_auth};
use matryx_entity::types::{Device, Session};
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
/// Implements Matrix Client-Server API logout endpoint with complete session invalidation.
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
    info!("Processing logout request");

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Logout failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let (user_id, device_id, access_token) = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Logout failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            (token_info.user_id.clone(), token_info.device_id.clone(), token_info.token.clone())
        },
        MatrixAuth::Server(_) => {
            warn!("Logout failed - server authentication not allowed for logout");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Logout failed - anonymous authentication not allowed for logout");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Logout request for user: {} device: {}", user_id, device_id);

    // Initialize repositories for database operations
    let session_repo = SessionRepository::new(state.db.clone());
    let device_repo = DeviceRepository::new(state.db.clone());

    // Invalidate session in database
    invalidate_user_session(&session_repo, &user_id, &device_id, &access_token).await?;

    // Delete device and associated device keys
    delete_user_device(&device_repo, &user_id, &device_id).await?;

    // Invalidate token in database
    invalidate_session_token(&session_repo, &access_token).await?;

    // Clean up any active LiveQuery subscriptions for this session
    cleanup_livequery_subscriptions(&state, &user_id, &device_id).await?;

    info!("Logout successful for user: {} device: {}", user_id, device_id);

    // Return empty JSON object as per Matrix specification
    Ok(Json(serde_json::json!({})))
}

/// Extract access token from Authorization header
fn extract_access_token(headers: &HeaderMap) -> Result<String, LogoutError> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| {
            warn!("Missing Authorization header");
            LogoutError::InvalidToken
        })?
        .to_str()
        .map_err(|_| {
            warn!("Invalid Authorization header format");
            LogoutError::InvalidToken
        })?;

    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        Ok(token.to_string())
    } else {
        warn!("Authorization header missing Bearer prefix");
        Err(LogoutError::InvalidToken)
    }
}

/// Invalidate user session in database
async fn invalidate_user_session(
    session_repo: &SessionRepository,
    user_id: &str,
    device_id: &str,
    access_token: &str,
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
    // Query to find and clean up any active LiveQuery subscriptions for this session
    let cleanup_query = "
        DELETE FROM livequery_subscriptions 
        WHERE user_id = $user_id AND device_id = $device_id
    ";

    match state
        .db
        .query(cleanup_query)
        .bind(("user_id", user_id.to_string()))
        .bind(("device_id", device_id.to_string()))
        .await
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_access_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer syt_test_token_123"));

        let result = extract_access_token(&headers).expect("Should extract valid token");
        assert_eq!(result, "syt_test_token_123");
    }

    #[test]
    fn test_extract_access_token_missing_header() {
        let headers = HeaderMap::new();
        let result = extract_access_token(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_access_token_invalid_format() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Invalid token_format"));

        let result = extract_access_token(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_access_token_missing_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("syt_test_token_123"));

        let result = extract_access_token(&headers);
        assert!(result.is_err());
    }
}
