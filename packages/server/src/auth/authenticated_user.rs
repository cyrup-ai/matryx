use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
};
use serde::{Deserialize, Serialize};

use crate::auth::MatrixAuthError;
use crate::state::AppState;

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
        // Check if user has membership in the room
        let membership_exists: Result<Option<bool>, _> = state.db
            .query("SELECT VALUE true FROM membership WHERE user_id = $user_id AND room_id = $room_id AND membership IN ['join', 'invite'] LIMIT 1")
            .bind(("user_id", self.user_id.clone()))
            .bind(("room_id", room_id))
            .await
            .and_then(|mut response| response.take(0));

        membership_exists.unwrap_or(Some(false)).unwrap_or(false)
    }

    /// Check if this user is an admin
    pub async fn is_admin(&self, state: &AppState) -> bool {
        let is_admin: Result<Option<bool>, _> = state
            .db
            .query("SELECT VALUE is_admin FROM user WHERE user_id = $user_id")
            .bind(("user_id", self.user_id.clone()))
            .await
            .and_then(|mut response| response.take(0));

        is_admin.unwrap_or(Some(false)).unwrap_or(false)
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

                let user_exists: Result<Option<bool>, _> = state.db
                    .query("SELECT VALUE true FROM user WHERE user_id = $user_id AND is_active = true LIMIT 1")
                    .bind(("user_id", user_id.clone()))
                    .await
                    .and_then(|mut response| response.take(0));

                if !user_exists
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .unwrap_or(false)
                {
                    return Err(StatusCode::UNAUTHORIZED);
                }

                // Verify device exists and is associated with user
                let device_exists: Result<Option<bool>, _> = state.db
                    .query("SELECT VALUE true FROM device WHERE device_id = $device_id AND user_id = $user_id LIMIT 1")
                    .bind(("device_id", device_id.clone()))
                    .bind(("user_id", user_id.clone()))
                    .await
                    .and_then(|mut response| response.take(0));

                if !device_exists
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .unwrap_or(false)
                {
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
