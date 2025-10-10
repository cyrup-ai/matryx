//! Matrix login client using MatrixHttpClient
//!
//! This module provides a structured client for Matrix login operations
//! using the centralized MatrixHttpClient infrastructure.

use crate::http_client::{MatrixHttpClient, HttpClientError};
use super::{LoginRequest, LoginResponse, LoginFlowsResponse};

/// Client for Matrix login operations using MatrixHttpClient
pub struct LoginClient {
    http_client: MatrixHttpClient,
}

impl LoginClient {
    /// Create new login client with MatrixHttpClient
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get available login flows from server
    ///
    /// Performs GET /_matrix/client/v3/login to discover supported
    /// authentication methods.
    ///
    /// # Returns
    /// - `Ok(LoginFlowsResponse)` with available login types
    /// - `Err(HttpClientError)` on network/server errors
    pub async fn get_login_flows(&self) -> Result<LoginFlowsResponse, HttpClientError> {
        self.http_client
            .get("/_matrix/client/v3/login")
            .await
    }

    /// Perform login request
    ///
    /// Sends POST /_matrix/client/v3/login with credentials.
    ///
    /// # Arguments
    /// * `request` - Login request with credentials and device info
    ///
    /// # Returns
    /// - `Ok(LoginResponse)` with access_token and user_id on success
    /// - `Err(HttpClientError::Matrix)` with M_FORBIDDEN on invalid credentials
    /// - `Err(HttpClientError::Matrix)` with M_LIMIT_EXCEEDED on rate limiting
    /// - `Err(HttpClientError::Network)` on connection failures
    pub async fn login(&self, request: &LoginRequest) -> Result<LoginResponse, HttpClientError> {
        let response: LoginResponse = self.http_client
            .post("/_matrix/client/v3/login", request)
            .await?;

        // Set the access token for future authenticated requests
        self.http_client.set_access_token(response.access_token.clone()).await;

        Ok(response)
    }

    /// Login with username and password (convenience method)
    ///
    /// Creates a password-type LoginRequest and performs login.
    ///
    /// # Arguments
    /// * `username` - User identifier (localpart without @server)
    /// * `password` - User's password
    /// * `device_id` - Optional device ID (server generates if None)
    /// * `device_display_name` - Optional human-readable device name
    pub async fn login_with_password(
        &self,
        username: &str,
        password: &str,
        device_id: Option<String>,
        device_display_name: Option<String>,
    ) -> Result<LoginResponse, HttpClientError> {
        let request = LoginRequest {
            login_type: "m.login.password".to_string(),
            user: Some(username.to_string()),
            password: Some(password.to_string()),
            device_id,
            initial_device_display_name: device_display_name,
            token: None,
            refresh_token: None,
        };

        self.login(&request).await
    }

    /// Login with token (convenience method)
    ///
    /// Used for SSO, application service, or pre-authenticated tokens.
    ///
    /// # Arguments
    /// * `token` - Pre-authenticated token from SSO or app service
    /// * `device_id` - Optional device ID
    /// * `device_display_name` - Optional human-readable device name
    pub async fn login_with_token(
        &self,
        token: &str,
        device_id: Option<String>,
        device_display_name: Option<String>,
    ) -> Result<LoginResponse, HttpClientError> {
        let request = LoginRequest {
            login_type: "m.login.token".to_string(),
            user: None,
            password: None,
            device_id,
            initial_device_display_name: device_display_name,
            token: Some(token.to_string()),
            refresh_token: None,
        };

        self.login(&request).await
    }
}
