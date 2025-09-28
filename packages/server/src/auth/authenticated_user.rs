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
        
        auth_repo.check_user_membership(&self.user_id, &room_id).await.unwrap_or_default()
    }

    /// Check if this user is an admin
    pub async fn is_admin(&self, state: &AppState) -> bool {
        let auth_repo = AuthRepository::new(state.db.clone());
        
        auth_repo.is_user_admin(&self.user_id).await.unwrap_or_default()
    }

    /// Get the Matrix user ID in proper format
    pub fn matrix_id(&self) -> String {
        if self.user_id.starts_with('@') {
            self.user_id.clone()
        } else {
            format!("@{}:{}", self.user_id, self.homeserver_name)
        }
    }

    /// Get the localpart of the user ID
    pub fn localpart(&self) -> &str {
        if let Some(localpart) = self.user_id.strip_prefix('@') {
            if let Some(colon_pos) = localpart.find(':') {
                &localpart[..colon_pos]
            } else {
                localpart
            }
        } else {
            &self.user_id
        }
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

                // Verify user exists in database using homeserver validation
                let expected_homeserver = state.session_service.get_homeserver_name();
                if !user_id.ends_with(&format!(":{}", expected_homeserver)) {
                    return Err(StatusCode::UNAUTHORIZED);
                }

                let auth_repo = AuthRepository::new(state.db.clone());

                // Check if user exists and is active
                let user_active = auth_repo.is_user_active(&user_id).await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if !user_active {
                    return Err(StatusCode::UNAUTHORIZED);
                }

                // Verify device exists and is associated with user
                let device_valid = auth_repo.validate_device(&device_id, &user_id).await
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
