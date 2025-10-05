use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
};
use serde::{Deserialize, Serialize};

use crate::auth::MatrixAuthError;
use crate::state::AppState;
use matryx_surrealdb::repository::auth::AuthRepository;

/// Represents an authenticated Matrix user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    pub homeserver_name: String,
}

impl AuthenticatedUser {
    pub fn new(
        user_id: String,
        device_id: String,
        access_token: String,
        homeserver_name: String,
    ) -> Self {
        Self { user_id, device_id, access_token, homeserver_name }
    }

    /// Check if this user can access a specific room
    pub async fn can_access_room(&self, state: &AppState, room_id: String) -> bool {
        let auth_repo = AuthRepository::new(state.db.clone());

        auth_repo
            .check_user_membership(&self.user_id, &room_id)
            .await
            .unwrap_or_default()
    }

    /// Check if this user can access a specific resource
    pub async fn can_access_resource(
        &self,
        state: &AppState,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<bool, crate::auth::MatrixAuthError> {
        match resource_type {
            "room" => {
                let auth_repo = AuthRepository::new(state.db.clone());
                auth_repo
                    .check_user_membership(&self.user_id, resource_id)
                    .await
                    .map_err(|e| crate::auth::MatrixAuthError::DatabaseError(e.to_string()))
            },
            "profile" => {
                // Users can access their own profile, admins can access any profile
                if resource_id == self.user_id {
                    Ok(true)
                } else {
                    self.is_admin(state).await
                }
            },
            _ => {
                // Default: deny access to unknown resource types
                Ok(false)
            },
        }
    }

    /// Check if this user has admin privileges
    pub async fn is_admin(&self, state: &AppState) -> Result<bool, crate::auth::MatrixAuthError> {
        let auth_repo = AuthRepository::new(state.db.clone());

        // Check if user has admin role in the database
        auth_repo
            .is_user_admin(&self.user_id)
            .await
            .map_err(|e| crate::auth::MatrixAuthError::DatabaseError(e.to_string()))
    }

    /// Get the device ID for this authenticated user
    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }

    /// Extract the localpart from the Matrix user ID (part before the colon)
    /// Returns None if the user ID is malformed
    pub fn localpart(&self) -> Option<&str> {
        // Matrix user IDs are in format @localpart:homeserver
        // Extract everything after @ and before :
        if let Some(stripped) = self.user_id.strip_prefix('@')
            && let Some(colon_pos) = stripped.find(':')
        {
            let localpart = &stripped[..colon_pos];
            // Validate that localpart is not empty
            if !localpart.is_empty() {
                return Some(localpart);
            }
        }
        // Return None for malformed user IDs
        None
    }

    /// Get the full Matrix user ID
    pub fn matrix_id(&self) -> &str {
        &self.user_id
    }

    /// Get the access token for this authenticated user
    pub fn get_access_token(&self) -> &str {
        &self.access_token
    }
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Parse Bearer token
        let access_token = if let Some(token) = auth_header.strip_prefix("Bearer ") {
            token.to_string()
        } else {
            return Err(StatusCode::UNAUTHORIZED);
        };

        // Verify token and get user information
        match state.session_service.validate_token(&access_token) {
            Ok(claims) => {
                let user_id = claims.matrix_user_id.ok_or(StatusCode::UNAUTHORIZED)?;
                let device_id = claims.matrix_device_id.ok_or(StatusCode::UNAUTHORIZED)?;

                // Create MatrixAuth for validation
                let matrix_token = crate::auth::MatrixAccessToken {
                    token: access_token.clone(),
                    user_id: user_id.clone(),
                    device_id: device_id.clone(),
                    expires_at: claims.exp,
                };
                let matrix_auth = crate::auth::MatrixAuth::User(matrix_token);

                // Use MatrixAuth methods for validation
                if matrix_auth.is_expired() {
                    return Err(StatusCode::UNAUTHORIZED);
                }

                // Verify user exists in database using homeserver validation
                let expected_homeserver = state.session_service.get_homeserver_name();
                if !user_id.ends_with(&format!(":{}", expected_homeserver)) {
                    return Err(StatusCode::UNAUTHORIZED);
                }

                let auth_repo = AuthRepository::new(state.db.clone());

                // Check if user exists and is active
                let user_active = auth_repo
                    .is_user_active(&user_id)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if !user_active {
                    return Err(StatusCode::UNAUTHORIZED);
                }

                // Verify device exists and is associated with user
                let device_valid = auth_repo
                    .validate_device(&device_id, &user_id)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if !device_valid {
                    return Err(StatusCode::UNAUTHORIZED);
                }

                Ok(AuthenticatedUser::new(
                    user_id,
                    device_id,
                    access_token,
                    state.homeserver_name.clone(),
                ))
            },
            Err(MatrixAuthError::SessionExpired) => Err(StatusCode::UNAUTHORIZED),
            Err(MatrixAuthError::UnknownToken) => Err(StatusCode::UNAUTHORIZED),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}
